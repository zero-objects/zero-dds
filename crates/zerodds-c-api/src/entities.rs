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
    /// Positional CDR field schema for the untyped C-FFI: set via
    /// `zerodds_cft_set_schema` so the filter expression evaluates against
    /// real decoded members instead of falling back to pass-through.
    pub schema: Mutex<Vec<CftField>>,
    /// Type extensibility of the payload (XTypes 1.3 §7.3.1.2.1). In XCDR2 an
    /// `@appendable`/`@mutable` aggregate is prefixed with a 4-byte DHEADER
    /// (§7.4.3.4.2) before the first member, so the positional schema must
    /// skip those 4 bytes; a `@final` type has no DHEADER (offset 0).
    pub extensibility: Mutex<CftExtensibility>,
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
    pub suspended: Mutex<bool>,
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
    /// Stable backing for PARTITION names emitted by `dw_get_qos`
    /// (see `qos_ffi::PartitionOutCache`).
    pub partition_out: Mutex<crate::qos_ffi::PartitionOutCache>,
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
    pub params: Vec<zerodds_sql_filter::Value>,
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
}

/// `RowAccess` over a raw CDR payload using a positional `CftField` schema.
/// Decodes the requested field lazily by walking the leading members in
/// declaration order until the named field is reached.
struct CdrRow<'a> {
    payload: &'a [u8],
    schema: &'a [CftField],
    extensibility: CftExtensibility,
}

impl zerodds_sql_filter::RowAccess for CdrRow<'_> {
    fn get(&self, path: &str) -> Option<zerodds_sql_filter::Value> {
        use zerodds_cdr::{BufferReader, Endianness};
        use zerodds_sql_filter::Value;
        // XCDR2 little-endian over the bare body (no encap header). The
        // same-runtime dispatch hands the reader the CDR body directly.
        let mut r = BufferReader::new(self.payload, Endianness::Little).xcdr2();
        // For an @appendable/@mutable aggregate the XCDR2 stream begins with a
        // 4-byte DHEADER (UInt32 object length, XTypes 1.3 §7.4.3.4.2) before
        // the first member; consume it so the schema offsets line up. A @final
        // type has no DHEADER (offset 0).
        if matches!(self.extensibility, CftExtensibility::Appendable) {
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
    /// Evaluates the filter against a sample payload (Spec §2.2.2.3.3).
    ///
    /// With a positional `schema`, the payload is decoded into a row and the
    /// SQL filter expression is genuinely applied (e.g. `seq > 4` rejects
    /// samples whose `seq` member is `<= 4`). Without a schema (untyped
    /// RawBytes topics that carry no field info), the spec-conformant
    /// pass-through applies: any field-lookup error → keep the sample.
    pub fn evaluate(&self, payload: &[u8]) -> bool {
        if self.schema.is_empty() {
            // Untyped pass-through: no field info, every lookup yields None.
            struct EmptyRow;
            impl zerodds_sql_filter::RowAccess for EmptyRow {
                fn get(&self, _path: &str) -> Option<zerodds_sql_filter::Value> {
                    None
                }
            }
            return self.expr.evaluate(&EmptyRow, &self.params).unwrap_or(true);
        }
        let row = CdrRow {
            payload,
            schema: &self.schema,
            extensibility: self.extensibility,
        };
        // A schema is present but a comparison failed to resolve (truncated
        // payload, type mismatch): DDS §2.2.2.3.3 — a sample that cannot be
        // evaluated is not delivered. `unwrap_or(false)` drops it.
        self.expr.evaluate(&row, &self.params).unwrap_or(false)
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
