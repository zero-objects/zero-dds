// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Publisher + DataWriter — das Sende-Ende der DCPS-API.
//!
//! Spec-Referenz: OMG DDS 1.4 §2.2.2.4 `Publisher`, §2.2.2.4.2
//! `DataWriter`.
//!
//! # Scope v1.2
//!
//! - `Publisher::create_datawriter<T>(topic, qos)` → `DataWriter<T>`.
//! - `DataWriter::write(&sample)` encodiert via `T::encode` und
//!   uebergibt an einen **in-memory Queue** (wiring zum
//!   ReliableWriter erfolgt in Runtime).
//! - Noch kein Matching gegen Remote-Reader.
//! - Noch kein QoS-Conflict-Check.
//!
//! # Thread-Safety
//!
//! `DataWriter` ist `Send`+`Sync` via `Arc<Mutex<_>>`. Mehrere
//! Application-Threads duerfen parallel `write()` aufrufen.

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

/// Publisher — Entity-Gruppe fuer DataWriter.
///
/// In DDS 1.4 hat der Publisher eigene QoS (Partition, Group-Data,
/// Presentation). v1.2 implementiert nur die API-Shape ohne
/// Partition-Matching.
#[derive(Debug)]
pub struct Publisher {
    pub(crate) inner: Arc<PublisherInner>,
}

pub(crate) struct PublisherInner {
    /// Mutable QoS. .1 (Entity-Lifecycle): set_qos prueft
    /// Immutability nach enable().
    #[cfg(feature = "std")]
    pub(crate) qos: std::sync::Mutex<PublisherQos>,
    #[cfg(not(feature = "std"))]
    #[allow(dead_code)]
    pub(crate) qos: PublisherQos,
    /// Entity-Lifecycle (DCPS §2.2.2.1).
    pub(crate) entity_state: alloc::sync::Arc<crate::entity::EntityState>,
    /// Runtime-Handle (wenn der Publisher von einem Live-Participant
    /// erzeugt wurde). None im offline-Modus → DataWriter fallen
    /// auf in-memory queue zurueck.
    #[cfg(feature = "std")]
    pub(crate) runtime: Option<Arc<DcpsRuntime>>,
    /// optionaler [`ArcPublisherListener`] + [`StatusMask`]
    /// (Spec §2.2.2.4.3.x set_listener / Bubble-Up §2.2.4.2.3).
    #[cfg(feature = "std")]
    pub(crate) listener: std::sync::Mutex<Option<(ArcPublisherListener, StatusMask)>>,
    /// Schwacher Back-Pointer auf den Participant — fuer Bubble-Up
    /// vom Publisher zum Participant. Wird von
    /// `DomainParticipant::create_publisher` gesetzt. `Weak`
    /// vermeidet einen Refcount-Cycle Participant↔Publisher.
    #[cfg(feature = "std")]
    pub(crate) participant:
        std::sync::Mutex<Option<alloc::sync::Weak<crate::participant::ParticipantInner>>>,
    /// `suspend_publications`-Flag (Spec §2.2.2.4.1.10). Wenn `true`,
    /// hat der Publisher die Hint gegeben, dass Writes gepuffert werden
    /// sollen — Writer kann das als Optimization-Hint nutzen
    /// (z.B. Coalescing). Nicht binnenkonsistent erzwungen, weil Spec
    /// es explizit als "hint to the Service" definiert.
    suspended: core::sync::atomic::AtomicBool,
    /// DataWriter-Handles (per `create_datawriter` getrackt) fuer
    /// rekursives `DomainParticipant::contains_entity`
    /// (Spec §2.2.2.2.1.10).
    #[cfg(feature = "std")]
    pub(crate) datawriters:
        std::sync::Mutex<alloc::vec::Vec<crate::instance_handle::InstanceHandle>>,
}

// Manueller Debug-Impl, weil `dyn PublisherListener` kein Debug
// implementiert. Wir geben nur "Some/None" und die Mask aus.
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

    /// Spec §2.2.2.2.1.10 — `true` wenn `handle` ein DataWriter ist,
    /// der ueber diesen Publisher erzeugt wurde.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn contains_writer(&self, handle: crate::instance_handle::InstanceHandle) -> bool {
        self.inner
            .datawriters
            .lock()
            .map(|v| v.contains(&handle))
            .unwrap_or(false)
    }

    /// setzt den `PublisherListener` + StatusMask. `None`
    /// loescht den Slot. Spec §2.2.2.4.3.x.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcPublisherListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.inner.entity_state.set_listener_mask(mask);
    }

    /// aktueller Listener-Klon, sofern vorhanden.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcPublisherListener> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Setzt den schwachen Back-Pointer auf den Participant. Wird
    /// vom `DomainParticipant::create_publisher` aufgerufen.
    #[cfg(feature = "std")]
    pub(crate) fn attach_participant(
        &self,
        participant: alloc::sync::Weak<crate::participant::ParticipantInner>,
    ) {
        if let Ok(mut slot) = self.inner.participant.lock() {
            *slot = Some(participant);
        }
    }

    /// Liefert die [`crate::listener_dispatch::WriterListenerChain`]
    /// fuer einen Writer dieses Publishers — Reader-Pfad-Pendant in
    /// Subscriber. Klont alle drei Listener-Stufen unter ihren
    /// Mutexen und gibt das Bundle frei zurueck (Lock-Discipline).
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

    /// Spec §2.2.2.4.1.10 `suspend_publications` — Hint an die Service,
    /// dass nachfolgende `write()`-Aufrufe gepuffert werden duerfen
    /// (z.B. fuer Coalescing). Hat keine Pflicht-Semantik fuer den
    /// Caller; der Flag ist via `is_suspended()` lesbar fuer die
    /// Writer-Implementation.
    ///
    /// Idempotent: ein zweites `suspend_publications()` ohne
    /// `resume_publications()` dazwischen ist erlaubt.
    pub fn suspend_publications(&self) {
        self.inner
            .suspended
            .store(true, core::sync::atomic::Ordering::Release);
    }

    /// Spec §2.2.2.4.1.11 `resume_publications` — Gegenstueck zu
    /// `suspend_publications`. Bei aktivem Suspend-Flag ist das
    /// Verhalten "Service can stop coalescing"; bei inaktivem Flag ist
    /// das ein No-Op.
    pub fn resume_publications(&self) {
        self.inner
            .suspended
            .store(false, core::sync::atomic::Ordering::Release);
    }

    /// `true` wenn `suspend_publications()` aktiv ist und
    /// `resume_publications()` noch nicht gerufen wurde. Wird vom
    /// Writer-Send-Pfad als Coalescing-Hint gelesen.
    #[must_use]
    pub fn is_suspended(&self) -> bool {
        self.inner
            .suspended
            .load(core::sync::atomic::Ordering::Acquire)
    }

    /// Spec §2.2.2.4.1.13 `copy_from_topic_qos` — kopiert die zwischen
    /// Topic- und DataWriter-Qos teilbaren Policies aus `topic_qos`
    /// nach `dw_qos`. Spec-Liste der gemeinsamen Policies (DCPS 1.4
    /// §2.2.2.4.1.13): DURABILITY, DEADLINE, LATENCY_BUDGET, LIVELINESS,
    /// RELIABILITY, DESTINATION_ORDER, HISTORY, RESOURCE_LIMITS,
    /// TRANSPORT_PRIORITY, LIFESPAN, OWNERSHIP.
    ///
    /// # Errors
    /// `DdsError::BadParameter` wenn das Resultat eine inkonsistente
    /// QoS-Kombination ergibt — wird vom QoS-Compatibility-Check des
    /// Caller-DataWriter validiert (analog `set_qos`).
    pub fn copy_from_topic_qos(
        dw_qos: &mut DataWriterQos,
        topic_qos: &crate::qos::TopicQos,
    ) -> Result<()> {
        // Die folgenden Felder sind in beiden QoS-Strukturen vorhanden
        // und werden 1:1 ueberschrieben. DataWriter-only Policies
        // (OWNERSHIP_STRENGTH, PARTITION, RESOURCE_LIMITS, HISTORY,
        // LIFESPAN, DEADLINE, LIVELINESS, OWNERSHIP) bleiben
        // unangetastet, weil TopicQos sie aktuell nicht traegt.
        // Wenn TopicQos um eines dieser Felder erweitert wird, MUSS
        // diese Liste mit-erweitert werden — Spec §2.2.2.4.1.13.
        dw_qos.durability = topic_qos.durability;
        dw_qos.reliability = topic_qos.reliability;
        Ok(())
    }

    /// Erzeugt einen typed `DataWriter<T>`. Spec §2.2.2.4.1.5
    /// `create_datawriter`.
    ///
    /// # Errors
    /// - `BadParameter` wenn `topic.type_name() != T::TYPE_NAME`
    ///   (sollte statisch garantiert sein, aber defensiv pruefen).
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
            // Live-Mode: registriere einen echten User-Writer bei
            // der Runtime. Matching und User-Data-Flow laufen ab
            // jetzt ueber SEDP + UDP.
            let reliable = qos.reliability.kind == ReliabilityKind::Reliable;
            let eid = rt.register_user_writer(crate::runtime::UserWriterConfig {
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
                // F-TYPES-3: Topic-Type-Identifier weitergeben.
                type_identifier: T::TYPE_IDENTIFIER.clone(),
                // D.5g — `None` = nutze Runtime-Default. Per-Writer-
                // Override via QoS-Policy ist TBD (`DataWriterQos::
                // representation`); die DataRepresentationQosPolicy
                // ist noch nicht in DataWriterQos modelliert.
                data_representation_offer: None,
            })?;
            let dw =
                DataWriter::new_live(topic.clone(), qos, self.inner.clone(), Arc::clone(rt), eid);
            // Spec §2.2.3.5 — bei Durability=Transient/Persistent das
            // Writer-eigene Backend an die Runtime weiterreichen, damit
            // der Match-Pfad beim ersten Late-Joiner-Match die
            // Backend-Samples in den HistoryCache re-injiziert (siehe
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
        // Propagiere zum Participant fuer rekursives contains_entity.
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
// Entity-Trait (DCPS §2.2.2.1) —
// ============================================================================

#[cfg(feature = "std")]
impl crate::entity::Entity for Publisher {
    type Qos = PublisherQos;

    fn get_qos(&self) -> Self::Qos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    fn set_qos(&self, qos: Self::Qos) -> Result<()> {
        // PublisherQos hat keine immutable Felder per DDS-Spec §2.2.3 —
        // Partition / GroupData / Presentation sind alle Changeable=YES.
        // Wir koennen also pre- und post-enable einfach uebernehmen.
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

/// Typed DataWriter — schickt Samples an alle matched Reader des Topics.
///
/// Zwei Modi:
/// - **Live** (`runtime: Some`, `entity_id: Some`): write() delegiert
///   an die Runtime → ReliableWriter → UDP.
/// - **Offline** (Offline-Fallback, Runtime=None): write() queued
///   in-memory; fuer Unit-Tests ohne Netz.
pub struct DataWriter<T: DdsType> {
    topic: Topic<T>,
    qos: Mutex<DataWriterQos>,
    /// Entity-Lifecycle (DCPS §2.2.2.1).
    entity_state: Arc<crate::entity::EntityState>,
    /// Parent-Publisher (clone des `Arc`) — fuer Bubble-Up zum
    /// Publisher- und Participant-Listener.
    publisher: Arc<PublisherInner>,
    /// optionaler `DataWriterListener` + `StatusMask`.
    #[cfg(feature = "std")]
    listener: Mutex<Option<(ArcDataWriterListener, StatusMask)>>,
    /// zuletzt gesehene Anzahl matched Reader. Wird vom
    /// `poll_status_changes` (lazy von Public-API-Pfaden gerufen)
    /// genutzt, um eine Delta-Detection fuer
    /// `on_publication_matched` zu fahren — Spec §2.2.4.2.4.4.
    #[cfg(feature = "std")]
    last_match_count: std::sync::atomic::AtomicI64,
    /// zuletzt gesehener offered_deadline_missed-Counter.
    #[cfg(feature = "std")]
    last_offered_deadline_missed: std::sync::atomic::AtomicU64,
    /// zuletzt gesehener liveliness_lost-Counter.
    #[cfg(feature = "std")]
    last_liveliness_lost: std::sync::atomic::AtomicU64,
    /// zuletzt gesehener offered_incompatible_qos.total_count.
    #[cfg(feature = "std")]
    last_offered_incompatible_qos: std::sync::atomic::AtomicI64,
    /// Offline Fallback-Queue.
    queue: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Drain-Notify-Pair (Spec §2.2.3.19 RESOURCE_LIMITS Reliable-Block).
    /// `write()` blockt am Condvar wenn die Queue full ist + RELIABLE +
    /// `max_blocking_time > 0`; `__drain_pending` notifies alle wartenden
    /// Writer-Threads.
    #[cfg(feature = "std")]
    drain_signal: Arc<std::sync::Condvar>,
    #[cfg(feature = "std")]
    runtime: Option<Arc<DcpsRuntime>>,
    #[cfg(feature = "std")]
    entity_id: Option<EntityId>,
    /// Instanz-Buchhaltung.
    #[cfg(feature = "std")]
    instances: InstanceTracker,
    /// Lokaler Publication-Handle — wird in `SampleInfo.publication_handle`
    /// auf der Reader-Seite eingetragen, sobald Live-Wiring greift.
    #[cfg(feature = "std")]
    publication_handle: InstanceHandle,
    /// Spec §2.2.3.5 DurabilityServiceQosPolicy: bei
    /// Durability=Transient/Persistent legt der Writer Samples zusaetzlich
    /// in einem Backend ab, damit Late-Joiner-Reader sie auch nach
    /// Writer-History-Cleanup beziehen koennen.
    #[cfg(feature = "std")]
    durability_backend: Option<Arc<dyn crate::durability_service::DurabilityBackend>>,
    /// Monoton steigende Writer-Sequenz fuer Durability-Backend-Storage
    /// (DDS 1.4 §2.2.3.5 + Backend-Replay-Reihenfolge).
    #[cfg(feature = "std")]
    durability_seq: std::sync::atomic::AtomicU64,
    /// Optional konfigurierter Flatdata-SlotBackend fuer Same-Host-
    /// Zero-Copy-Pfad (`zerodds-flatdata-1.0` §4.1 + §8.1). Wird via
    /// `set_flat_backend` (siehe `flatdata_integration`-Modul) gesetzt;
    /// bei `None` faellt `write_flat()` auf den klassischen UDP-Pfad
    /// zurueck.
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

    /// Liefert das Durability-Backend (None bei Volatile/TransientLocal).
    /// Test-/Inspektions-Hilfsfunktion — Spec §2.2.3.5.
    #[doc(hidden)]
    #[cfg(feature = "std")]
    #[must_use]
    pub fn durability_backend(
        &self,
    ) -> Option<Arc<dyn crate::durability_service::DurabilityBackend>> {
        self.durability_backend.clone()
    }

    /// Spec §2.2.3.5: bei Durability=Transient legt der Writer ein
    /// In-Memory-Backend an. Persistent ohne Root-Pfad wird nicht
    /// auto-konfiguriert — Caller muss `set_durability_backend`
    /// Default-Pfad fuer Persistent: `ZERODDS_DURABILITY_DIR` Env-Var,
    /// sonst `std::env::temp_dir().join("zerodds-durability")`. Caller
    /// kann den Pfad ueber die Env-Var ueberschreiben fuer Production-
    /// Deployments (z.B. `/var/lib/zerodds/durability`).
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
        // Wir leiten den Publication-Handle aus der EntityId ab — das macht ihn
        // ueber Test-Runs reproduzierbar und vermeidet eine Pool-Kollision mit
        // Instance-Handles. Spec sagt ohnehin nur, dass es sich um ein opakes
        // u64 handelt.
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

    /// Topic, an das gesendet wird.
    #[must_use]
    pub fn topic(&self) -> &Topic<T> {
        &self.topic
    }

    /// setzt den `DataWriterListener` + StatusMask. `None`
    /// loescht den Slot. Spec §2.2.2.4.2.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcDataWriterListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.entity_state.set_listener_mask(mask);
    }

    /// aktueller Listener-Klon, sofern vorhanden.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcDataWriterListener> {
        self.listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Snapshot der Bubble-Up-Kette (Writer → Publisher → Participant).
    /// Fuer Hot-Path-Listener-Dispatch genutzt.
    #[cfg(feature = "std")]
    #[must_use]
    pub(crate) fn listener_chain(&self) -> crate::listener_dispatch::WriterListenerChain {
        let writer = self
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)));
        // Wir reichen den Writer-Klon an den Publisher-Snapshot weiter,
        // der Publisher- und Participant-Stage befuellt.
        let pub_handle = Publisher {
            inner: Arc::clone(&self.publisher),
        };
        pub_handle.snapshot_writer_chain(writer)
    }

    /// Aktuelle QoS (cloned, .1).
    #[must_use]
    pub fn qos(&self) -> DataWriterQos {
        self.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// Sendet einen Sample an alle matched Reader.
    ///
    /// **Spec §2.2.3.19 RESOURCE_LIMITS Reliable-Block:** Wenn der
    /// lokale Writer-Cache `max_samples` erreicht hat UND
    /// Reliability=RELIABLE UND `max_blocking_time > 0`, blockt
    /// `write()` bis Reader ACK den Slot freigibt oder das Timeout
    /// abgelaufen ist. Im Best-Effort-Mode oder mit
    /// `max_blocking_time = 0` schlaegt `write()` sofort fehl mit
    /// `OutOfResources`.
    ///
    /// # Errors
    /// - `WireError` wenn `T::encode` fehlschlaegt.
    /// - `OutOfResources` wenn die Queue full ist + Best-Effort/keine
    ///   Block-Zeit, oder wenn der Block-Timeout vor einem Drain abgelaufen ist.
    /// - `PreconditionNotMet` bei Lock-Poisoning.
    pub fn write(&self, sample: &T) -> Result<()> {
        let mut buf = Vec::new();
        sample.encode(&mut buf).map_err(|e| DdsError::WireError {
            message: e.to_string(),
        })?;
        #[cfg(feature = "metrics")]
        crate::metrics::inc_sample_written(self.topic.name());
        #[cfg(feature = "metrics")]
        crate::metrics::record_sample_size(self.topic.name(), buf.len());
        // Spec §2.2.3.5 DurabilityServiceQosPolicy: Sample zusaetzlich
        // ins Backend ablegen (Transient/Persistent), damit Late-Joiner-
        // Reader nach Writer-History-Cleanup noch beziehen koennen.
        #[cfg(feature = "std")]
        if let Some(backend) = self.durability_backend.as_ref() {
            let key_bytes = Self::keyhash_and_holder(sample)
                .map(|(kh, _)| kh)
                .unwrap_or([0u8; 16]);
            // Monotone Writer-Sequenz fuer Backend-Replay-Reihenfolge
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
        // Live-Mode: delegiere an Runtime → ReliableWriter → UDP.
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            return rt.write_user_sample(eid, buf);
        }
        // Offline-Fallback: in-memory queue mit RESOURCE_LIMITS-Block.
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
                // Reliable + max_blocking_time > 0 → wait_timeout am Condvar.
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
                    // sonst spurious wakeup → weiter warten.
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

    /// Anzahl bisher geschriebener Samples. Test-Hilfsfunktion,
    /// in Runtime durch echte HistoryCache-Counter ersetzt.
    #[must_use]
    pub fn samples_pending(&self) -> usize {
        self.queue.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// Anzahl matched Remote-Reader. Im Offline-Mode immer 0.
    ///
    /// Spec: OMG DDS 1.4 §2.2.2.4.2.11 `get_matched_subscriptions`.
    /// Dort wird eine Liste zurueckgegeben; liefert nur
    /// den Count, die volle Liste kommt mit Listener-Callbacks.
    ///
    /// Seiteneffekt — bei einer Aenderung des Matched-Count
    /// gegenueber dem letzten Aufruf wird `on_publication_matched`
    /// via Bubble-Up-Kette gefeuert (Spec §2.2.4.2.4.4).
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

    /// vergleicht den `current`-Count mit
    /// `last_match_count` und feuert `on_publication_matched` wenn
    /// sich der Wert geaendert hat. Initial ist `last_match_count == -1`,
    /// d.h. der erste Aufruf mit n>=0 triggert immer.
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

    /// Delta-Detect fuer `on_offered_deadline_missed`.
    /// Liest den Counter aus der Runtime und feuert den Listener bei
    /// Delta. Spec §2.2.4.2.4.1.
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

    /// Delta-Detect fuer `on_liveliness_lost`. Spec §2.2.4.2.4.3.
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

    /// Delta-Detect fuer `on_offered_incompatible_qos`.
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

    /// Blockiert, bis mindestens `min_count` Remote-Reader matched
    /// sind oder `timeout` verstreicht. Event-driven via Runtime-Condvar
    /// (D.5e Phase-1) — wakup direkt wenn SEDP einen Match propagiert,
    /// kein 20-ms-Polling mehr.
    ///
    /// Verwandt zu OMG DDS 1.4 §2.2.2.4.2.22 `wait_for_acknowledgments`,
    /// aber fokussiert auf Matching statt ACK. Deckt den typischen
    /// Producer-Pattern "erst Writer anlegen, dann auf Subscriber warten,
    /// dann schreiben" ab.
    ///
    /// # Errors
    /// [`DdsError::Timeout`] wenn `min_count` im Zeitfenster nicht
    /// erreicht wird.
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

    /// Counter fuer offered-Deadline-Verletzungen (Spec
    /// §2.2.4.2.9 `OFFERED_DEADLINE_MISSED_STATUS`). Monoton steigend;
    /// steigt um 1 pro abgelaufenem Deadline-Fenster ohne Write.
    /// Im Offline-Mode oder bei `deadline=INFINITE` immer 0.
    ///
    /// feuert ggf. `on_offered_deadline_missed` ueber die
    /// Bubble-Up-Kette bei Delta gegenueber dem letzten Aufruf.
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

    /// Counter fuer LivelinessLost-Detections (Spec §2.2.4.2.10).
    /// Triggert ggf. `on_liveliness_lost` via Bubble-Up.
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

    /// aktueller `OfferedIncompatibleQosStatus` (Spec
    /// §2.2.4.2.4.2). Triggert ggf. `on_offered_incompatible_qos`.
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

    /// pollt alle Statuses einmal und feuert pending Listener.
    /// Convenient Helper fuer Tests + periodische Tick-Aufrufer.
    #[cfg(feature = "std")]
    pub fn drive_listeners(&self) {
        let _ = self.matched_subscription_count();
        let _ = self.offered_deadline_missed_count();
        let _ = self.liveliness_lost_count();
        let _ = self.offered_incompatible_qos_status();
    }

    /// Manual-Liveliness-Assert. Spec §2.2.2.4.2.20
    /// `assert_liveliness`. Setzt den `last_liveliness_assert`-Timestamp;
    /// bei Automatic-Liveliness no-op (jeder write asserts ohnehin).
    #[cfg(feature = "std")]
    pub fn assert_liveliness(&self) {
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            rt.assert_writer_liveliness_eid(eid);
        }
    }

    /// Blockiert, bis alle matched Remote-Reader alle bis jetzt
    /// geschriebenen Samples acknowledgt haben, oder `timeout` abläuft.
    ///
    /// Spec: OMG DDS 1.4 §2.2.2.4.2.22 `wait_for_acknowledgments`.
    /// Im Offline-Mode und ohne gematchte Reader sofort `Ok(())`.
    ///
    /// # Errors
    /// [`DdsError::Timeout`] wenn nicht alle Samples im Zeitfenster
    /// acknowledgt sind.
    #[cfg(feature = "std")]
    pub fn wait_for_acknowledgments(&self, timeout: core::time::Duration) -> Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let all_acked = match (&self.runtime, self.entity_id) {
                (Some(rt), Some(eid)) => rt.user_writer_all_acknowledged(eid),
                _ => true, // offline: nichts zu bestaetigen
            };
            if all_acked {
                return Ok(());
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(DdsError::Timeout);
            }
            // D.5e Phase-1: event-driven via Runtime-ack-event-Cvar.
            if let Some(rt) = self.runtime.as_ref() {
                let _ = rt.wait_ack_event(deadline - now);
            } else {
                std::thread::sleep(core::time::Duration::from_millis(20));
            }
        }
    }

    /// Nimmt alle pending Samples aus der Offline-Queue heraus. Nur
    /// fuer Tests; wird mit Live-Mode-Wiring entfernt.
    #[doc(hidden)]
    pub fn __drain_pending(&self) -> Vec<Vec<u8>> {
        let drained = self
            .queue
            .lock()
            .map(|mut q| core::mem::take(&mut *q))
            .unwrap_or_default();
        // Spec §2.2.3.19: Drain-Signal an wartende `write()`-Threads.
        #[cfg(feature = "std")]
        self.drain_signal.notify_all();
        drained
    }

    // ========================================================================
    // Instance-API.4 / DDS 1.4 §2.2.2.4.2.{5,7,10,13,14}
    // ========================================================================

    /// Lokaler Publication-Handle dieses DataWriters (Spec §2.2.2.5.1.11).
    /// Wird im `publication_handle`-Feld des `SampleInfo` mitgegeben.
    /// **Achtung**: das ist NICHT derselbe Handle wie der Entity-
    /// `InstanceHandle` (Spec §2.2.2.1.1) — siehe [`Self::instance_handle`].
    #[cfg(feature = "std")]
    #[must_use]
    pub fn publication_handle(&self) -> InstanceHandle {
        self.publication_handle
    }

    /// Spec §2.2.2.1.1 `get_instance_handle` — Entity-Identifier
    /// dieses DataWriters fuer Vergleiche via
    /// `DomainParticipant::contains_entity`.
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.entity_state.instance_handle()
    }

    /// Liefert den aktuellen [`InstanceTracker`] (geteilt mit der
    /// internen Buchhaltung). Hauptsaechlich fuer Tests / Inspection.
    #[cfg(feature = "std")]
    #[must_use]
    /// Liefert (Runtime, EntityId), wenn der Writer im Live-Mode laeuft.
    /// Cross-Crate-Hook fuer FFI-Layer (zerodds-c-api), die
    /// rt.write_user_lifecycle direkt aufrufen muessen.
    #[doc(hidden)]
    #[cfg(feature = "std")]
    pub fn runtime_handle(&self) -> Option<(Arc<DcpsRuntime>, EntityId)> {
        match (&self.runtime, self.entity_id) {
            (Some(rt), Some(eid)) => Some((Arc::clone(rt), eid)),
            _ => None,
        }
    }

    /// Liefert den geteilten Instance-Tracker des Writers (Test- und
    /// Inspection-Helper, Spec §2.2.2.4.2.5+ Lifecycle-Buchhaltung).
    pub fn instance_tracker(&self) -> InstanceTracker {
        self.instances.clone()
    }

    /// Berechnet den KeyHash + den PLAIN_CDR2-BE-Key-Holder fuer ein
    /// Sample. Liefert `None` fuer non-keyed Topics.
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

    /// Registriert eine Instanz beim DataWriter und liefert ihren
    /// stabilen [`InstanceHandle`] zurueck. Spec §2.2.2.4.2.5
    /// `register_instance`.
    ///
    /// Fuer non-keyed Topics liefert der Aufruf [`HANDLE_NIL`] zurueck
    /// (jedes Sample ist seine eigene "Instanz", die Spec sagt explizit
    /// dass register/unregister/dispose hier optional sind).
    ///
    /// # Errors
    /// Aktuell kann der Aufruf nicht fehlschlagen. Spaeter (Live-Mode)
    /// koennen Resource-Limits hier einen `OutOfResources`-Fehler
    /// liefern.
    #[cfg(feature = "std")]
    pub fn register_instance(&self, instance: &T) -> Result<InstanceHandle> {
        self.register_instance_w_timestamp(instance, get_current_time())
    }

    /// Wie `register_instance`, aber mit explizitem Timestamp.
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

    /// Macht aus einem Sample-Wert den dazugehoerigen lokalen
    /// [`InstanceHandle`], oder [`HANDLE_NIL`] wenn unbekannt /
    /// non-keyed. Spec §2.2.2.4.2.14 `lookup_instance`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn lookup_instance(&self, instance: &T) -> InstanceHandle {
        let Some((kh, _)) = Self::keyhash_and_holder(instance) else {
            return HANDLE_NIL;
        };
        self.instances.lookup(&kh).unwrap_or(HANDLE_NIL)
    }

    /// Entfernt die Instanz aus dem Writer-Set (Spec §2.2.2.4.2.7).
    /// Setzt den Lifecycle-Zustand auf `NOT_ALIVE_NO_WRITERS`, sobald
    /// der letzte Writer sich abgemeldet hat.
    ///
    /// # Errors
    /// `BadParameter` wenn `handle` nicht zur Instanz von `instance`
    /// passt (Spec verlangt diese Konsistenz-Pruefung). Wenn
    /// `handle == HANDLE_NIL`, wird der Handle aus `instance`
    /// abgeleitet.
    #[cfg(feature = "std")]
    pub fn unregister_instance(&self, instance: &T, handle: InstanceHandle) -> Result<()> {
        self.unregister_instance_w_timestamp(instance, handle, get_current_time())
    }

    /// Wie `unregister_instance`, aber mit Timestamp. Spec §2.2.2.4.2.8.
    ///
    /// Spec §2.2.3.21 WriterDataLifecycle: wenn
    /// `autodispose_unregistered_instances=true` (Default), wird die
    /// Instanz zusaetzlich zum Unregister auch disposed — Reader sehen
    /// dann sowohl `NOT_ALIVE_DISPOSED` als auch `NOT_ALIVE_NO_WRITERS`.
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
        // Wire-Side (Spec §9.6.3.9 PID_STATUS_INFO): an alle matched Reader
        // einen Lifecycle-Marker schicken. Bei autodispose=true setzen wir
        // beide Bits, sonst nur UNREGISTERED.
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

    /// Disposed eine Instanz (Spec §2.2.2.4.2.10). Markiert sie als
    /// `NOT_ALIVE_DISPOSED`; Reader sehen dann ein Sample mit
    /// `valid_data == false`.
    ///
    /// # Errors
    /// Wie `unregister_instance`.
    #[cfg(feature = "std")]
    pub fn dispose(&self, instance: &T, handle: InstanceHandle) -> Result<()> {
        self.dispose_w_timestamp(instance, handle, get_current_time())
    }

    /// Wie `dispose`, aber mit Timestamp. Spec §2.2.2.4.2.11.
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
        // Wire-Side (Spec §9.6.3.9 PID_STATUS_INFO).
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

    /// Gibt den Sample-Wert mit nur den `@key`-Feldern befuellt zurueck
    /// (Spec §2.2.2.4.2.13 `get_key_value`). Implementierung: wir
    /// rekonstruieren `T` via `decode` aus dem gespeicherten
    /// PLAIN_CDR2-BE-Key-Holder. Damit das funktioniert, muss `T::decode`
    /// einen Key-only-Stream akzeptieren — fuer einfache Records ist das
    /// trivialerweise der Fall.
    ///
    /// # Errors
    /// * `BadParameter` wenn der Handle unbekannt ist.
    /// * `WireError` wenn der Key-Holder nicht via `T::decode`
    ///   rekonstruierbar ist.
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

    /// Hilfsfunktion: `handle == HANDLE_NIL` → aus `instance` ableiten.
    /// Sonst: pruefen, dass `handle` zur Instanz von `instance` passt.
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

    /// Schreibt ein Sample mit explizitem Timestamp (Spec §2.2.2.4.2.16
    /// `write_w_timestamp`) und aktualisiert die Instanz-Buchhaltung.
    ///
    /// # Errors
    /// Wie [`Self::write`].
    #[cfg(feature = "std")]
    pub fn write_w_timestamp(&self, sample: &T, timestamp: Time) -> Result<()> {
        // Auto-Register: wenn die Instanz noch nicht bekannt ist,
        // registrieren wir sie implizit (Spec §2.2.2.4.2.16 erlaubt das).
        if let Some((kh, holder)) = Self::keyhash_and_holder(sample) {
            if self.instances.lookup(&kh).is_none() {
                self.instances.register(kh, holder, Some(timestamp));
            } else {
                // Bei Re-Activation nach Dispose / NoWriters bumpt das
                // register den Generation-Counter, fuegt aber gleichzeitig
                // einen Writer-Count hinzu, den wir nicht wollen — daher
                // direkt wieder dekrementieren.
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
    /// RESOURCE_LIMITS, OWNERSHIP, LIVELINESS sind Changeable=NO post-enable.
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

// ---- Boxed-typemapped variant, damit Publisher eine heterogene
// Writer-Liste halten kann (Live-Mode-Vorbereitung) ----
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
        // Mask wird nach EntityState gespiegelt.
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

        // 0 → 0 (Initial-Aufruf, AtomicI64 ist -1, also Delta da).
        w.poll_publication_matched(0);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 1);
        // 0 → 1 (Aenderung).
        w.poll_publication_matched(1);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 2);
        // 1 → 1 (kein Delta).
        w.poll_publication_matched(1);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 2);
        // 1 → 2.
        w.poll_publication_matched(2);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 3);
        // 2 → 1 (Reader weg).
        w.poll_publication_matched(1);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn poll_publication_matched_with_no_listener_is_noop() {
        let p = Publisher::new(PublisherQos::default(), None);
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        // Kein Listener gesetzt — darf weder panicken noch Delta-State
        // korrumpieren.
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
        // Kein Writer-Listener → Publisher empfaengt.
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
        p.suspend_publications(); // zweiter Call ist No-Op
        assert!(p.is_suspended());
    }

    #[test]
    fn resume_without_suspend_is_noop() {
        let p = Publisher::new(PublisherQos::default(), None);
        // Spec §2.2.2.4.1.11 — resume ohne aktivem suspend ist No-Op.
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
        // Setze etwas anderes, damit wir die Aenderung sehen.
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
        // Erst beide Slots fuellen (no block).
        w.write(&s).unwrap();
        w.write(&s).unwrap();
        assert_eq!(w.samples_pending(), 2);

        // Dritter write blockt; in einem zweiten Thread drain wir nach 50ms.
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
        // Zweiter write hat Reliable + 50ms Block; ohne drain → Timeout.
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
        // max_samples = -1 (LENGTH_UNLIMITED) → kein Cap, kein Block.
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
        // Setze ownership_strength auf einen konkreten Wert; sollte
        // nach copy unangetastet bleiben (kein TopicQos-Counterpart).
        dw.ownership_strength.value = 42;
        Publisher::copy_from_topic_qos(&mut dw, &topic).unwrap();
        assert_eq!(dw.ownership_strength.value, 42);
    }
}
