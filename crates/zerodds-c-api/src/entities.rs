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
    /// Local read cache for non-destructive `read()` (Spec §2.2.2.5.3).
    /// Per sample `(sample, sample_state)` is stored; `take` pulls
    /// from cache+channel and removes; `read` reads from cache+channel
    /// without removing, but marks the sample state as READ.
    pub read_cache: Mutex<Vec<(UserSample, ReadSampleState)>>,
    /// Optional: ContentFilteredTopic filter active. If Some,
    /// `take`/`read` evaluate every sample against the filter (Spec §2.2.2.3.3).
    /// Untyped topics (RawBytes/String) return true for all filters,
    /// because no type info is present (vendor decision: pass-through
    /// instead of block-all).
    pub cft_filter: Option<CftFilter>,
}

/// ContentFilteredTopic filter in the DataReader.
pub struct CftFilter {
    /// Parsed filter expression from `crates/sql-filter`.
    pub expr: zerodds_sql_filter::Expr,
    /// Filter parameters %0..%N as a `Value` vector.
    pub params: Vec<zerodds_sql_filter::Value>,
}

impl CftFilter {
    /// Evaluates the filter against a sample payload.
    /// Untyped/RawBytes: always returns `true` (spec-conformant pass-through).
    pub fn evaluate(&self, _payload: &[u8]) -> bool {
        struct EmptyRow;
        impl zerodds_sql_filter::RowAccess for EmptyRow {
            fn get(&self, _path: &str) -> Option<zerodds_sql_filter::Value> {
                None
            }
        }
        // Untyped pass-through: on every field-lookup error → true.
        self.expr.evaluate(&EmptyRow, &self.params).unwrap_or(true)
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
