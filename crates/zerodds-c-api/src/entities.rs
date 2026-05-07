// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Opaque Entity-Handle-Types fuer die spec-konforme C-FFI-Surface
//! (DDS-Spec §2.2.2 + DDS-PSM-Cxx 1.0 §7.2.1).
//!
//! Die Handles sind `Box<...>`-allokierte Wrapper um die Rust-DCPS-
//! Entities. Pro `*_create()` wird der Box in einen Pointer umgewandelt;
//! `*_destroy()` ruft `Box::from_raw` und droppt damit auch den
//! gehaltenen `Arc<DcpsRuntime>`-Klon, sodass die Runtime aufraeumt
//! wenn der letzte Handle entfernt ist.
//!
//! ## Architektur-Hinweis
//!
//! Auf C-FFI-Ebene ist die Spec-Hierarchie (Factory → Participant →
//! Publisher/Subscriber/Topic → DataWriter/DataReader) als opake
//! Handle-Kette modelliert. Intern collapsen DataWriter und DataReader
//! auf den `DcpsRuntime::create_user_writer/reader`-Pfad mit
//! Caller-supplied `type_name`-String, weil `DataWriter<T>`-Generik
//! kein Runtime-`type_name` erlaubt (`T::TYPE_NAME` ist `const`).
//!
//! Publisher/Subscriber sind organisatorische Container die QoS-Defaults
//! plus eine Liste der enthaltenen Writers/Readers tracken — sie bleiben
//! voll spec-konform an der API-Oberflaeche (incl. presentation,
//! partition, group_data, suspend/resume).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::{Mutex, mpsc};

use zerodds_dcps::factory::DomainParticipantFactoryQos;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_dcps::runtime::{DcpsRuntime, UserSample};
use zerodds_rtps::wire_types::EntityId;

// ============================================================================
// DomainParticipantFactory — Singleton
// ============================================================================

/// Singleton DomainParticipantFactory (Spec §2.2.2.2.1).
pub struct ZeroDdsDomainParticipantFactory {
    /// Default-QoS fuer neu zu erzeugende Participants.
    pub default_participant_qos: Mutex<DomainParticipantQos>,
    /// Factory-eigene QoS (entity_factory.autoenable_created_entities).
    pub factory_qos: Mutex<DomainParticipantFactoryQos>,
    /// Registry aller aktiven Participants (fuer lookup_participant).
    pub participants: Mutex<Vec<*mut ZeroDdsDomainParticipant>>,
}

// SAFETY: Pointer in `participants` werden nur unter `Mutex` zugegriffen
// und referenzieren `Box`-allokierte ZeroDdsDomainParticipants, die
// `Send + Sync` sind durch den enthaltenen `Arc<DcpsRuntime>`.
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsDomainParticipantFactory {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsDomainParticipantFactory {}

impl ZeroDdsDomainParticipantFactory {
    /// Liefert die globale Singleton-Instance.
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
    /// Live-Wrapper auf die hochstufige `dcps::DomainParticipant`-Form.
    /// Erlaubt direkten Zugriff auf `ignore_*`, `contains_entity`,
    /// `assert_liveliness`, Discovery-Listings etc.
    pub dp: DomainParticipant,
    /// Runtime-Klon (extrahiert via `dp.runtime()`) fuer den
    /// User-Writer/Reader-Pfad. None wenn offline.
    pub rt: Option<Arc<DcpsRuntime>>,
    /// Domain-ID (cached, aus dp.domain_id()).
    pub domain_id: u32,
    pub default_topic_qos: Mutex<TopicQos>,
    pub default_publisher_qos: Mutex<PublisherQos>,
    pub default_subscriber_qos: Mutex<SubscriberQos>,
    /// Topics die ueber diesen Participant erzeugt wurden.
    pub topics: Mutex<Vec<*mut ZeroDdsTopic>>,
    pub publishers: Mutex<Vec<*mut ZeroDdsPublisher>>,
    pub subscribers: Mutex<Vec<*mut ZeroDdsSubscriber>>,
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
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsTopic {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsTopic {}

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
    pub suspended: Mutex<bool>,
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
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsSubscriber {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsSubscriber {}

// ============================================================================
// DataWriter
// ============================================================================

/// DataWriter (Spec §2.2.2.4.2).
pub struct ZeroDdsDataWriter {
    pub publisher: *mut ZeroDdsPublisher,
    pub topic: *mut ZeroDdsTopic,
    pub rt: Arc<DcpsRuntime>,
    pub eid: EntityId,
    pub qos: Mutex<DataWriterQos>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsDataWriter {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsDataWriter {}

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
    /// Lokaler Read-Cache fuer non-destructive `read()` (Spec §2.2.2.5.3).
    /// Per Sample wird `(sample, sample_state)` gespeichert; `take` zieht
    /// aus Cache+Channel und entfernt; `read` liest aus Cache+Channel
    /// ohne zu entfernen, markiert aber Sample-State als READ.
    pub read_cache: Mutex<Vec<(UserSample, ReadSampleState)>>,
    /// Optional: ContentFilteredTopic-Filter aktiv. Wenn Some, evaluiert
    /// `take`/`read` jedes Sample gegen den Filter (Spec §2.2.2.3.3).
    /// Untyped Topics (RawBytes/String) liefern fuer alle Filter true,
    /// weil keine Type-Info vorhanden ist (Vendor-Decision: pass-through
    /// statt block-all).
    pub cft_filter: Option<CftFilter>,
}

/// ContentFilteredTopic-Filter im DataReader.
pub struct CftFilter {
    /// Geparste Filter-Expression aus `crates/sql-filter`.
    pub expr: zerodds_sql_filter::Expr,
    /// Filter-Parameter %0..%N als `Value`-Vektor.
    pub params: Vec<zerodds_sql_filter::Value>,
}

impl CftFilter {
    /// Evaluiert den Filter gegen einen Sample-Payload.
    /// Untyped/RawBytes: liefert immer `true` (Spec-konform pass-through).
    pub fn evaluate(&self, _payload: &[u8]) -> bool {
        struct EmptyRow;
        impl zerodds_sql_filter::RowAccess for EmptyRow {
            fn get(&self, _path: &str) -> Option<zerodds_sql_filter::Value> {
                None
            }
        }
        // Untyped pass-through: bei jedem field-lookup-Error → true.
        self.expr.evaluate(&EmptyRow, &self.params).unwrap_or(true)
    }
}

/// Lokaler Sample-State im Read-Cache (Spec §2.2.2.5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadSampleState {
    /// Frisch aus Channel — sample_state = NOT_READ (Bit 2).
    NotRead,
    /// Bereits via `read()` gelesen — sample_state = READ (Bit 1).
    Read,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsDataReader {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsDataReader {}

// ============================================================================
// Helper-Funktionen fuer FFI-Pointer-Validierung
// ============================================================================

/// Sicherer Cast `*mut T` → `&T` mit NULL-Check.
///
/// # Safety
/// Caller muss garantieren dass `p` entweder NULL ist oder auf eine
/// valide `Box<T>`-Allocation zeigt, die noch nicht freigegeben ist.
pub unsafe fn handle_ref<T>(p: *mut T) -> Option<&'static T> {
    if p.is_null() {
        None
    } else {
        // SAFETY: NULL-Check oben + Caller-Kontrakt.
        Some(unsafe { &*p })
    }
}
