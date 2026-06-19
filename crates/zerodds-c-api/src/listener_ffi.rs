// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Listener callback path for all entity types (DDS 1.4 §2.2.4 +
//! vendor extension `docs/specs/zerodds-listener-callbacks-1.1.md`).
//!
//! ## Architecture (vendor spec)
//!
//! The OMG DDS spec defines listeners as **classes** in C++/Java/C#
//! (`DataWriterListener`, `DataReaderListener`, etc.) — for C-FFI there
//! is no normative path. This vendor extension defines listeners as
//! **C function-pointer tables** (`vtable`-style) with a
//! `void* user_data` slot for caller state.
//!
//! ### Pattern
//!
//! ```c
//! typedef struct {
//!     void (*on_offered_deadline_missed)(void* user_data, /* args */);
//!     void (*on_publication_matched)(void* user_data, /* args */);
//!     /* ... */
//!     void* user_data;  // Opaker Caller-Pointer.
//! } zerodds_DataWriterListener;
//!
//! zerodds_dw_set_listener(dw, &my_listener, status_mask);
//! ```
//!
//! Bei C++/C#/Java-Bindings wrappt jede Sprache diese vtable in
//! ihre native Listener-Class — siehe `crates/cpp/include/dds/pub/
//! DataWriterListener.hpp` etc.
//!
//! ### Status-Mask
//!
//! Die `status_mask` filtert welche Callbacks aktiv sind (Spec
//! §2.2.4.2.1). Bits gemass `dds::core::status::StatusKind`.
//!
//! ### Threading
//!
//! Callbacks werden vom Runtime-Worker-Thread (UDP-RX) gefeuert.
//! Caller-Code muss thread-safe sein.
//!
//! ### Lifetime
//!
//! Der Listener-Pointer bleibt im Besitz des Callers — FFI hat den
//! Pointer nur weak. Caller muss `set_listener(NULL)` rufen bevor
//! die Listener-Struktur aus dem Scope geht.
//!
//! ### Spec-Mapping
//!
//! | DDS-Spec Listener (§2.2.4) | C-FFI Funktions-Pointer | Status-Bit |
//! |----------------------------|--------------------------|------------|
//! | `on_offered_deadline_missed` | DataWriterListener::on_offered_deadline_missed | OFFERED_DEADLINE_MISSED |
//! | `on_publication_matched` | DataWriterListener::on_publication_matched | PUBLICATION_MATCHED |
//! | `on_data_available` | DataReaderListener::on_data_available | DATA_AVAILABLE |
//! | `on_subscription_matched` | DataReaderListener::on_subscription_matched | SUBSCRIPTION_MATCHED |
//! | ... (10 mehr) | ... | ... |

use core::ffi::c_int;
use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::ZeroDdsStatus;
use crate::entities::{
    ZeroDdsDataReader, ZeroDdsDataWriter, ZeroDdsDomainParticipant, ZeroDdsPublisher,
    ZeroDdsSubscriber, ZeroDdsTopic,
};

// ============================================================================
// DomainParticipant Listener
// ============================================================================

/// `DomainParticipantListener` — Spec §2.2.4.2.1.
///
/// Alle Felder sind optional (NULL-Pointer = Callback ignoriert).
/// `user_data` wird unveraendert an jeden Callback gereicht.
#[repr(C)]
#[derive(Default)]
pub struct ZeroDdsDomainParticipantListener {
    /// Caller-State.
    pub user_data: *mut core::ffi::c_void,
    /// Inconsistent-Topic.
    pub on_inconsistent_topic: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsTopic)>,
    /// Aggregator: Data-On-Readers (Subscriber-Bubble-Up).
    pub on_data_on_readers: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsSubscriber)>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsDomainParticipantListener {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsDomainParticipantListener {}

// ============================================================================
// Publisher Listener
// ============================================================================

/// `PublisherListener` — Spec §2.2.4.2.2.
#[repr(C)]
#[derive(Default)]
pub struct ZeroDdsPublisherListener {
    /// Caller-State.
    pub user_data: *mut core::ffi::c_void,
    /// Aggregator-Pfad: alle Writer-Status-Bubble-Ups.
    pub on_offered_deadline_missed:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter)>,
    /// Liveliness lost.
    pub on_liveliness_lost: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter)>,
    /// Offered-Incompatible-QoS.
    pub on_offered_incompatible_qos:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter)>,
    /// Publication-matched.
    pub on_publication_matched:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter)>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsPublisherListener {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsPublisherListener {}

// ============================================================================
// Subscriber Listener
// ============================================================================

/// `SubscriberListener` — Spec §2.2.4.2.3.
#[repr(C)]
#[derive(Default)]
pub struct ZeroDdsSubscriberListener {
    /// Caller-State.
    pub user_data: *mut core::ffi::c_void,
    /// Data-On-Readers (Sub-Aggregator).
    pub on_data_on_readers: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsSubscriber)>,
    /// Sample-Lost (Reader-Bubble-Up).
    pub on_sample_lost: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Sample-Rejected.
    pub on_sample_rejected: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Liveliness-Changed.
    pub on_liveliness_changed:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Subscription-Matched.
    pub on_subscription_matched:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Requested-Deadline-Missed.
    pub on_requested_deadline_missed:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Requested-Incompatible-QoS.
    pub on_requested_incompatible_qos:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Data-Available.
    pub on_data_available: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsSubscriberListener {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsSubscriberListener {}

// ============================================================================
// Topic Listener
// ============================================================================

/// `TopicListener` — Spec §2.2.4.2.4.
#[repr(C)]
#[derive(Default)]
pub struct ZeroDdsTopicListener {
    /// Caller-State.
    pub user_data: *mut core::ffi::c_void,
    /// Inconsistent-Topic.
    pub on_inconsistent_topic: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsTopic)>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsTopicListener {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsTopicListener {}

// ============================================================================
// DataWriter Listener
// ============================================================================

/// `DataWriterListener` — Spec §2.2.4.2.5.
#[repr(C)]
#[derive(Default)]
pub struct ZeroDdsDataWriterListener {
    /// Caller-State.
    pub user_data: *mut core::ffi::c_void,
    /// Liveliness-Lost.
    pub on_liveliness_lost: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter)>,
    /// Offered-Deadline-Missed.
    pub on_offered_deadline_missed:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter)>,
    /// Offered-Incompatible-QoS.
    pub on_offered_incompatible_qos:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter)>,
    /// Publication-Matched.
    pub on_publication_matched:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter)>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsDataWriterListener {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsDataWriterListener {}

// ============================================================================
// DataReader Listener
// ============================================================================

/// `DataReaderListener` — Spec §2.2.4.2.6.
#[repr(C)]
#[derive(Default)]
pub struct ZeroDdsDataReaderListener {
    /// Caller-State.
    pub user_data: *mut core::ffi::c_void,
    /// Data-Available.
    pub on_data_available: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Sample-Rejected.
    pub on_sample_rejected: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Liveliness-Changed.
    pub on_liveliness_changed:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Requested-Deadline-Missed.
    pub on_requested_deadline_missed:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Requested-Incompatible-QoS.
    pub on_requested_incompatible_qos:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Subscription-Matched.
    pub on_subscription_matched:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    /// Sample-Lost.
    pub on_sample_lost: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsDataReaderListener {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsDataReaderListener {}

// ============================================================================
// Listener storage per entity (per ZeroDds wrapper)
//
// Listeners are registered in a static
// `OnceLock<Mutex<HashMap<*mut Entity, ListenerInfo>>>`, and the caller
// drives them via the polling hook `zerodds_poll_listeners()` — see
// further down in this file.
// ============================================================================

use std::collections::HashMap;
use std::sync::OnceLock;

/// Pro DataWriter/Reader-Pointer: zuletzt gesehene Counter-Snapshots
/// pro Status-Kind. Delta-Detection im poll_listeners-Pfad nutzt das.
#[derive(Debug, Default, Clone, Copy)]
struct WriterCounters {
    matched_count: usize,
    liveliness_lost: u64,
    offered_deadline_missed: u64,
    offered_incompatible_qos_total: i32,
}

#[derive(Debug, Default, Clone, Copy)]
struct ReaderCounters {
    matched_count: usize,
    sample_lost: u64,
    requested_deadline_missed: u64,
    requested_incompatible_qos_total: i32,
    /// `alive_count + not_alive_count` — a delta means liveliness changed.
    liveliness_change: u64,
    sample_rejected_total: i32,
    /// Monotonic delivered-sample count — a delta means data available.
    samples_delivered: u64,
}

/// Per-poll delta set computed for one DataWriter target.
#[derive(Debug, Default, Clone, Copy)]
struct WriterDelta {
    matched: bool,
    liveliness_lost: bool,
    deadline: bool,
    qos: bool,
}

/// Per-poll delta set computed for one DataReader target.
#[derive(Debug, Default, Clone, Copy)]
struct ReaderDelta {
    matched: bool,
    sample_lost: bool,
    deadline: bool,
    qos: bool,
    liveliness: bool,
    rejected: bool,
    data: bool,
}

struct ListenerRegistry {
    dp: Mutex<HashMap<usize, (*const ZeroDdsDomainParticipantListener, u32)>>,
    pub_: Mutex<HashMap<usize, (*const ZeroDdsPublisherListener, u32)>>,
    sub: Mutex<HashMap<usize, (*const ZeroDdsSubscriberListener, u32)>>,
    topic: Mutex<HashMap<usize, (*const ZeroDdsTopicListener, u32)>>,
    dw: Mutex<HashMap<usize, (*const ZeroDdsDataWriterListener, u32)>>,
    dr: Mutex<HashMap<usize, (*const ZeroDdsDataReaderListener, u32)>>,
    /// Per DW pointer: last-seen counters (for delta detection).
    dw_counters: Mutex<BTreeMap<usize, WriterCounters>>,
    /// Per DR pointer: last-seen counters.
    dr_counters: Mutex<BTreeMap<usize, ReaderCounters>>,
    /// Per topic/DP observer pointer: last-seen `inconsistent_topic_count`
    /// of the associated runtime (a delta is an inconsistent-topic event).
    ic_counters: Mutex<HashMap<usize, u64>>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ListenerRegistry {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ListenerRegistry {}

fn registry() -> &'static ListenerRegistry {
    static REG: OnceLock<ListenerRegistry> = OnceLock::new();
    REG.get_or_init(|| ListenerRegistry {
        dp: Mutex::new(HashMap::new()),
        pub_: Mutex::new(HashMap::new()),
        sub: Mutex::new(HashMap::new()),
        topic: Mutex::new(HashMap::new()),
        dw: Mutex::new(HashMap::new()),
        dr: Mutex::new(HashMap::new()),
        dw_counters: Mutex::new(BTreeMap::new()),
        dr_counters: Mutex::new(BTreeMap::new()),
        ic_counters: Mutex::new(HashMap::new()),
    })
}

// ============================================================================
// `*_set_listener` / `*_get_listener` API
// ============================================================================

/// `dp_set_listener`. NULL = clear.
///
/// # Safety
/// `p` valide; `l` darf NULL sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_set_listener(
    p: *mut ZeroDdsDomainParticipant,
    l: *const ZeroDdsDomainParticipantListener,
    status_mask: u32,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let key = p as usize;
    if let Ok(mut g) = registry().dp.lock() {
        if l.is_null() {
            g.remove(&key);
        } else {
            g.insert(key, (l, status_mask));
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// `pub_set_listener`.
///
/// # Safety
/// `pub_` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_set_listener(
    pub_: *mut ZeroDdsPublisher,
    l: *const ZeroDdsPublisherListener,
    status_mask: u32,
) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let key = pub_ as usize;
    if let Ok(mut g) = registry().pub_.lock() {
        if l.is_null() {
            g.remove(&key);
        } else {
            g.insert(key, (l, status_mask));
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// `sub_set_listener`.
///
/// # Safety
/// `sub` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_set_listener(
    sub: *mut ZeroDdsSubscriber,
    l: *const ZeroDdsSubscriberListener,
    status_mask: u32,
) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let key = sub as usize;
    if let Ok(mut g) = registry().sub.lock() {
        if l.is_null() {
            g.remove(&key);
        } else {
            g.insert(key, (l, status_mask));
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// `topic_set_listener`.
///
/// # Safety
/// `t` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_set_listener(
    t: *mut ZeroDdsTopic,
    l: *const ZeroDdsTopicListener,
    status_mask: u32,
) -> c_int {
    if t.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let key = t as usize;
    if let Ok(mut g) = registry().topic.lock() {
        if l.is_null() {
            g.remove(&key);
        } else {
            g.insert(key, (l, status_mask));
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// `dw_set_listener`.
///
/// # Safety
/// `dw` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_set_listener(
    dw: *mut ZeroDdsDataWriter,
    l: *const ZeroDdsDataWriterListener,
    status_mask: u32,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let key = dw as usize;
    if let Ok(mut g) = registry().dw.lock() {
        if l.is_null() {
            g.remove(&key);
            // Counter-Cache auch aufraeumen
            if let Ok(mut c) = registry().dw_counters.lock() {
                c.remove(&key);
            }
        } else {
            g.insert(key, (l, status_mask));
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// `dr_set_listener`.
///
/// # Safety
/// `dr` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_set_listener(
    dr: *mut ZeroDdsDataReader,
    l: *const ZeroDdsDataReaderListener,
    status_mask: u32,
) -> c_int {
    if dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let key = dr as usize;
    if let Ok(mut g) = registry().dr.lock() {
        if l.is_null() {
            g.remove(&key);
            if let Ok(mut c) = registry().dr_counters.lock() {
                c.remove(&key);
            }
        } else {
            g.insert(key, (l, status_mask));
        }
    }
    ZeroDdsStatus::Ok as c_int
}

// ============================================================================
// `*_get_listener` Pendants — RC1: liefern den letzten gesetzten Pointer
// (Caller-owned, Lifetime-Verantwortung beim Caller).
// ============================================================================

/// `dp_get_listener` — liefert den zuletzt gesetzten Pointer oder NULL.
///
/// # Safety
/// `p` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_listener(
    p: *mut ZeroDdsDomainParticipant,
) -> *const ZeroDdsDomainParticipantListener {
    if p.is_null() {
        return core::ptr::null();
    }
    registry()
        .dp
        .lock()
        .ok()
        .and_then(|g| g.get(&(p as usize)).map(|(ptr, _)| *ptr))
        .unwrap_or(core::ptr::null())
}

/// `dw_get_listener`.
///
/// # Safety
/// `dw` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_listener(
    dw: *mut ZeroDdsDataWriter,
) -> *const ZeroDdsDataWriterListener {
    if dw.is_null() {
        return core::ptr::null();
    }
    registry()
        .dw
        .lock()
        .ok()
        .and_then(|g| g.get(&(dw as usize)).map(|(ptr, _)| *ptr))
        .unwrap_or(core::ptr::null())
}

/// `dr_get_listener`.
///
/// # Safety
/// `dr` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_listener(
    dr: *mut ZeroDdsDataReader,
) -> *const ZeroDdsDataReaderListener {
    if dr.is_null() {
        return core::ptr::null();
    }
    registry()
        .dr
        .lock()
        .ok()
        .and_then(|g| g.get(&(dr as usize)).map(|(ptr, _)| *ptr))
        .unwrap_or(core::ptr::null())
}

// ============================================================================
// Active-Wireup via expliziter Poll-API
// ============================================================================
//
// The caller invokes `zerodds_poll_listeners()` periodically (typically in
// the main loop, every 50-200ms). The function walks all registered
// listeners, compares the current status counters against the last-seen
// value (cached per entity pointer), and fires the callbacks on a delta.
// The status-mask filter is applied.
//
// Threading contract: callbacks fire on the caller thread of the poll call.
// Cross-language bindings can integrate this into their own event loop
// (Tokio tick, .NET timer, Python asyncio, JS setInterval).

/// Status bits per `dds::core::status::StatusKind` (DDS 1.4 §2.3.2).
const STATUS_INCONSISTENT_TOPIC: u32 = 1 << 0;
const STATUS_OFFERED_DEADLINE_MISSED: u32 = 1 << 1;
const STATUS_REQUESTED_DEADLINE_MISSED: u32 = 1 << 2;
const STATUS_OFFERED_INCOMPATIBLE_QOS: u32 = 1 << 5;
const STATUS_REQUESTED_INCOMPATIBLE_QOS: u32 = 1 << 6;
const STATUS_SAMPLE_LOST: u32 = 1 << 7;
const STATUS_SAMPLE_REJECTED: u32 = 1 << 8;
const STATUS_DATA_ON_READERS: u32 = 1 << 9;
const STATUS_DATA_AVAILABLE: u32 = 1 << 10;
const STATUS_LIVELINESS_LOST: u32 = 1 << 11;
const STATUS_LIVELINESS_CHANGED: u32 = 1 << 12;
const STATUS_PUBLICATION_MATCHED: u32 = 1 << 13;
const STATUS_SUBSCRIPTION_MATCHED: u32 = 1 << 14;

/// Reads the current writer status counters.
///
/// # Safety
/// `dw_ptr` must point to a valid, registered DataWriter.
unsafe fn read_writer_counters(dw_ptr: *mut ZeroDdsDataWriter) -> WriterCounters {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dwr = unsafe { &*dw_ptr };
    WriterCounters {
        matched_count: dwr.rt.user_writer_matched_count(dwr.eid),
        liveliness_lost: dwr.rt.user_writer_liveliness_lost(dwr.eid),
        offered_deadline_missed: dwr.rt.user_writer_offered_deadline_missed(dwr.eid),
        offered_incompatible_qos_total: dwr
            .rt
            .user_writer_offered_incompatible_qos(dwr.eid)
            .total_count,
    }
}

/// Reads the current reader status counters.
///
/// # Safety
/// `dr_ptr` must point to a valid, registered DataReader.
unsafe fn read_reader_counters(dr_ptr: *mut ZeroDdsDataReader) -> ReaderCounters {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr_ptr };
    let (_, alive, not_alive) = drr.rt.user_reader_liveliness_status(drr.eid);
    ReaderCounters {
        matched_count: drr.rt.user_reader_matched_count(drr.eid),
        sample_lost: drr.rt.user_reader_sample_lost(drr.eid),
        requested_deadline_missed: drr.rt.user_reader_requested_deadline_missed(drr.eid),
        requested_incompatible_qos_total: drr
            .rt
            .user_reader_requested_incompatible_qos(drr.eid)
            .total_count,
        liveliness_change: alive.saturating_add(not_alive),
        sample_rejected_total: drr.rt.user_reader_sample_rejected(drr.eid).total_count,
        samples_delivered: drr.rt.user_reader_samples_delivered(drr.eid),
    }
}

fn writer_delta(now: &WriterCounters, prev: &WriterCounters) -> WriterDelta {
    WriterDelta {
        matched: now.matched_count != prev.matched_count,
        liveliness_lost: now.liveliness_lost > prev.liveliness_lost,
        deadline: now.offered_deadline_missed > prev.offered_deadline_missed,
        qos: now.offered_incompatible_qos_total > prev.offered_incompatible_qos_total,
    }
}

fn reader_delta(now: &ReaderCounters, prev: &ReaderCounters) -> ReaderDelta {
    ReaderDelta {
        matched: now.matched_count != prev.matched_count,
        sample_lost: now.sample_lost > prev.sample_lost,
        deadline: now.requested_deadline_missed > prev.requested_deadline_missed,
        qos: now.requested_incompatible_qos_total > prev.requested_incompatible_qos_total,
        liveliness: now.liveliness_change > prev.liveliness_change,
        rejected: now.sample_rejected_total > prev.sample_rejected_total,
        data: now.samples_delivered > prev.samples_delivered,
    }
}

/// Fires the four writer callbacks of a writer vtable for a delta. Shared
/// by the DataWriter level (own delta) and the Publisher aggregator (child
/// delta) — both structs have field-identical writer callback slots.
#[allow(clippy::too_many_arguments)]
fn fire_writer_vtable(
    on_publication_matched: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter)>,
    on_liveliness_lost: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter)>,
    on_offered_deadline_missed: Option<
        extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter),
    >,
    on_offered_incompatible_qos: Option<
        extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataWriter),
    >,
    user_data: *mut core::ffi::c_void,
    mask: u32,
    d: WriterDelta,
    dw_ptr: *mut ZeroDdsDataWriter,
) -> usize {
    let mut n = 0;
    if d.matched && (mask & STATUS_PUBLICATION_MATCHED) != 0 {
        if let Some(cb) = on_publication_matched {
            cb(user_data, dw_ptr);
            n += 1;
        }
    }
    if d.liveliness_lost && (mask & STATUS_LIVELINESS_LOST) != 0 {
        if let Some(cb) = on_liveliness_lost {
            cb(user_data, dw_ptr);
            n += 1;
        }
    }
    if d.deadline && (mask & STATUS_OFFERED_DEADLINE_MISSED) != 0 {
        if let Some(cb) = on_offered_deadline_missed {
            cb(user_data, dw_ptr);
            n += 1;
        }
    }
    if d.qos && (mask & STATUS_OFFERED_INCOMPATIBLE_QOS) != 0 {
        if let Some(cb) = on_offered_incompatible_qos {
            cb(user_data, dw_ptr);
            n += 1;
        }
    }
    n
}

/// Fires the seven reader callbacks of a reader vtable for a delta. Shared
/// by the DataReader level and the Subscriber aggregator (field-identical
/// reader callback slots). `on_data_on_readers` is NOT part of this vtable
/// and is handled separately (set semantics).
#[allow(clippy::too_many_arguments)]
fn fire_reader_vtable(
    on_subscription_matched: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    on_sample_lost: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    on_requested_deadline_missed: Option<
        extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader),
    >,
    on_requested_incompatible_qos: Option<
        extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader),
    >,
    on_liveliness_changed: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    on_sample_rejected: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    on_data_available: Option<extern "C" fn(*mut core::ffi::c_void, *mut ZeroDdsDataReader)>,
    user_data: *mut core::ffi::c_void,
    mask: u32,
    d: ReaderDelta,
    dr_ptr: *mut ZeroDdsDataReader,
) -> usize {
    let mut n = 0;
    if d.matched && (mask & STATUS_SUBSCRIPTION_MATCHED) != 0 {
        if let Some(cb) = on_subscription_matched {
            cb(user_data, dr_ptr);
            n += 1;
        }
    }
    if d.sample_lost && (mask & STATUS_SAMPLE_LOST) != 0 {
        if let Some(cb) = on_sample_lost {
            cb(user_data, dr_ptr);
            n += 1;
        }
    }
    if d.deadline && (mask & STATUS_REQUESTED_DEADLINE_MISSED) != 0 {
        if let Some(cb) = on_requested_deadline_missed {
            cb(user_data, dr_ptr);
            n += 1;
        }
    }
    if d.qos && (mask & STATUS_REQUESTED_INCOMPATIBLE_QOS) != 0 {
        if let Some(cb) = on_requested_incompatible_qos {
            cb(user_data, dr_ptr);
            n += 1;
        }
    }
    if d.liveliness && (mask & STATUS_LIVELINESS_CHANGED) != 0 {
        if let Some(cb) = on_liveliness_changed {
            cb(user_data, dr_ptr);
            n += 1;
        }
    }
    if d.rejected && (mask & STATUS_SAMPLE_REJECTED) != 0 {
        if let Some(cb) = on_sample_rejected {
            cb(user_data, dr_ptr);
            n += 1;
        }
    }
    if d.data && (mask & STATUS_DATA_AVAILABLE) != 0 {
        if let Some(cb) = on_data_available {
            cb(user_data, dr_ptr);
            n += 1;
        }
    }
    n
}

/// Snapshots the child DataWriter pointers of a publisher (lock released
/// before the callback dispatch).
///
/// # Safety
/// `pub_key` must point to a valid, registered Publisher.
unsafe fn collect_publisher_writers(pub_key: usize) -> Vec<usize> {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pubr = unsafe { &*(pub_key as *mut ZeroDdsPublisher) };
    pubr.datawriters
        .lock()
        .map(|g| {
            g.iter()
                .filter(|p| !p.is_null())
                .map(|p| *p as usize)
                .collect()
        })
        .unwrap_or_default()
}

/// Snapshots the child DataReader pointers of a subscriber.
///
/// # Safety
/// `sub_key` must point to a valid, registered Subscriber.
unsafe fn collect_subscriber_readers(sub_key: usize) -> Vec<usize> {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let subr = unsafe { &*(sub_key as *mut ZeroDdsSubscriber) };
    subr.datareaders
        .lock()
        .map(|g| {
            g.iter()
                .filter(|p| !p.is_null())
                .map(|p| *p as usize)
                .collect()
        })
        .unwrap_or_default()
}

/// Snapshots `(subscriber pointer, [DataReader pointers])` for every
/// subscriber of a participant — for the DP aggregator (`on_data_on_readers`).
///
/// # Safety
/// `dp_key` must point to a valid, registered DomainParticipant.
unsafe fn collect_participant_subscriber_readers(dp_key: usize) -> Vec<(usize, Vec<usize>)> {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dp = unsafe { &*(dp_key as *mut ZeroDdsDomainParticipant) };
    let subs: Vec<usize> = dp
        .subscribers
        .lock()
        .map(|g| {
            g.iter()
                .filter(|p| !p.is_null())
                .map(|p| *p as usize)
                .collect()
        })
        .unwrap_or_default();
    subs.into_iter()
        // SAFETY: subscriber pointers come from the participant list and are
        // valid under the same caller contract.
        .map(|sk| (sk, unsafe { collect_subscriber_readers(sk) }))
        .collect()
}

/// Reads the `inconsistent_topic_count` of the runtime behind a participant
/// pointer (0 when offline/NULL).
///
/// # Safety
/// `participant` may be NULL or must point to a valid participant.
unsafe fn participant_inconsistent_count(participant: *mut ZeroDdsDomainParticipant) -> u64 {
    if participant.is_null() {
        return 0;
    }
    // SAFETY: NULL-checked above + caller contract.
    let dp = unsafe { &*participant };
    dp.rt
        .as_ref()
        .map(|rt| rt.inconsistent_topic_count())
        .unwrap_or(0)
}

/// Polls all registered listeners and fires callbacks on status counter
/// deltas. Each entity level (DataWriter/DataReader) and each aggregator
/// level (Publisher/Subscriber/DomainParticipant/Topic) fires independently
/// — multi-bind semantics, no first-match suppression. Returns the number
/// of callbacks fired.
///
/// # Safety
/// The caller must guarantee that every entity pointer registered in the
/// registry (including the contained child entities) and every listener
/// pointer is still valid (not freed via `*_destroy` without a preceding
/// `set_listener(NULL)`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_poll_listeners() -> usize {
    let mut fired: usize = 0;
    let reg = registry();

    // ---- Registry snapshots (locks released before any callback) ----
    let dw_listeners: Vec<(usize, *const ZeroDdsDataWriterListener, u32)> = reg
        .dw
        .lock()
        .map(|g| g.iter().map(|(k, (l, m))| (*k, *l, *m)).collect())
        .unwrap_or_default();
    let dr_listeners: Vec<(usize, *const ZeroDdsDataReaderListener, u32)> = reg
        .dr
        .lock()
        .map(|g| g.iter().map(|(k, (l, m))| (*k, *l, *m)).collect())
        .unwrap_or_default();
    let pub_listeners: Vec<(usize, *const ZeroDdsPublisherListener, u32)> = reg
        .pub_
        .lock()
        .map(|g| g.iter().map(|(k, (l, m))| (*k, *l, *m)).collect())
        .unwrap_or_default();
    let sub_listeners: Vec<(usize, *const ZeroDdsSubscriberListener, u32)> = reg
        .sub
        .lock()
        .map(|g| g.iter().map(|(k, (l, m))| (*k, *l, *m)).collect())
        .unwrap_or_default();
    let dp_listeners: Vec<(usize, *const ZeroDdsDomainParticipantListener, u32)> = reg
        .dp
        .lock()
        .map(|g| g.iter().map(|(k, (l, m))| (*k, *l, *m)).collect())
        .unwrap_or_default();
    let topic_listeners: Vec<(usize, *const ZeroDdsTopicListener, u32)> = reg
        .topic
        .lock()
        .map(|g| g.iter().map(|(k, (l, m))| (*k, *l, *m)).collect())
        .unwrap_or_default();

    // ---- Child pointer snapshots for the aggregator levels ----
    // SAFETY: pointers come from the registry; caller contract (see doc).
    let pub_children: Vec<Vec<usize>> = pub_listeners
        .iter()
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        .map(|(pk, _, _)| unsafe { collect_publisher_writers(*pk) })
        .collect();
    let sub_children: Vec<Vec<usize>> = sub_listeners
        .iter()
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        .map(|(sk, _, _)| unsafe { collect_subscriber_readers(*sk) })
        .collect();
    let dp_children: Vec<Vec<(usize, Vec<usize>)>> = dp_listeners
        .iter()
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        .map(|(dk, _, _)| unsafe { collect_participant_subscriber_readers(*dk) })
        .collect();

    // ---- Compute writer deltas for every observed DataWriter ----
    let mut w_keys: BTreeMap<usize, ()> = BTreeMap::new();
    for (k, _, _) in &dw_listeners {
        w_keys.insert(*k, ());
    }
    for ch in &pub_children {
        for k in ch {
            w_keys.insert(*k, ());
        }
    }
    let mut wnow: BTreeMap<usize, WriterCounters> = BTreeMap::new();
    let mut wdelta: BTreeMap<usize, WriterDelta> = BTreeMap::new();
    {
        let prev = reg.dw_counters.lock();
        for k in w_keys.keys() {
            let ptr = *k as *mut ZeroDdsDataWriter;
            if ptr.is_null() {
                continue;
            }
            // SAFETY: pointer from the registry; caller contract.
            let now = unsafe { read_writer_counters(ptr) };
            let p = prev
                .as_ref()
                .ok()
                .and_then(|m| m.get(k).copied())
                .unwrap_or_default();
            wdelta.insert(*k, writer_delta(&now, &p));
            wnow.insert(*k, now);
        }
    }

    // ---- Compute reader deltas for every observed DataReader ----
    let mut r_keys: BTreeMap<usize, ()> = BTreeMap::new();
    for (k, _, _) in &dr_listeners {
        r_keys.insert(*k, ());
    }
    for ch in &sub_children {
        for k in ch {
            r_keys.insert(*k, ());
        }
    }
    for subs in &dp_children {
        for (_, drs) in subs {
            for k in drs {
                r_keys.insert(*k, ());
            }
        }
    }
    let mut rnow: BTreeMap<usize, ReaderCounters> = BTreeMap::new();
    let mut rdelta: BTreeMap<usize, ReaderDelta> = BTreeMap::new();
    {
        let prev = reg.dr_counters.lock();
        for k in r_keys.keys() {
            let ptr = *k as *mut ZeroDdsDataReader;
            if ptr.is_null() {
                continue;
            }
            // SAFETY: pointer from the registry; caller contract.
            let now = unsafe { read_reader_counters(ptr) };
            let p = prev
                .as_ref()
                .ok()
                .and_then(|m| m.get(k).copied())
                .unwrap_or_default();
            rdelta.insert(*k, reader_delta(&now, &p));
            rnow.insert(*k, now);
        }
    }

    // ---- DataWriter level ----
    for (key, listener, mask) in &dw_listeners {
        if listener.is_null() {
            continue;
        }
        let Some(d) = wdelta.get(key) else { continue };
        // SAFETY: listener checked non-null; caller contract.
        let l = unsafe { &**listener };
        fired += fire_writer_vtable(
            l.on_publication_matched,
            l.on_liveliness_lost,
            l.on_offered_deadline_missed,
            l.on_offered_incompatible_qos,
            l.user_data,
            *mask,
            *d,
            *key as *mut ZeroDdsDataWriter,
        );
    }

    // ---- Publisher aggregator ----
    for (i, (_, listener, mask)) in pub_listeners.iter().enumerate() {
        if listener.is_null() {
            continue;
        }
        // SAFETY: caller contract.
        let l = unsafe { &**listener };
        for dwk in &pub_children[i] {
            let Some(d) = wdelta.get(dwk) else { continue };
            fired += fire_writer_vtable(
                l.on_publication_matched,
                l.on_liveliness_lost,
                l.on_offered_deadline_missed,
                l.on_offered_incompatible_qos,
                l.user_data,
                *mask,
                *d,
                *dwk as *mut ZeroDdsDataWriter,
            );
        }
    }

    // ---- DataReader level ----
    for (key, listener, mask) in &dr_listeners {
        if listener.is_null() {
            continue;
        }
        let Some(d) = rdelta.get(key) else { continue };
        // SAFETY: caller contract.
        let l = unsafe { &**listener };
        fired += fire_reader_vtable(
            l.on_subscription_matched,
            l.on_sample_lost,
            l.on_requested_deadline_missed,
            l.on_requested_incompatible_qos,
            l.on_liveliness_changed,
            l.on_sample_rejected,
            l.on_data_available,
            l.user_data,
            *mask,
            *d,
            *key as *mut ZeroDdsDataReader,
        );
    }

    // ---- Subscriber aggregator (reader callbacks + data_on_readers) ----
    for (i, (sk, listener, mask)) in sub_listeners.iter().enumerate() {
        if listener.is_null() {
            continue;
        }
        // SAFETY: caller contract.
        let l = unsafe { &**listener };
        let mut any_data = false;
        for drk in &sub_children[i] {
            let Some(d) = rdelta.get(drk) else { continue };
            fired += fire_reader_vtable(
                l.on_subscription_matched,
                l.on_sample_lost,
                l.on_requested_deadline_missed,
                l.on_requested_incompatible_qos,
                l.on_liveliness_changed,
                l.on_sample_rejected,
                l.on_data_available,
                l.user_data,
                *mask,
                *d,
                *drk as *mut ZeroDdsDataReader,
            );
            if d.data {
                any_data = true;
            }
        }
        if any_data && (*mask & STATUS_DATA_ON_READERS) != 0 {
            if let Some(cb) = l.on_data_on_readers {
                cb(l.user_data, *sk as *mut ZeroDdsSubscriber);
                fired += 1;
            }
        }
    }

    // ---- DomainParticipant aggregator (data_on_readers + inconsistent_topic) ----
    for (i, (dk, listener, mask)) in dp_listeners.iter().enumerate() {
        if listener.is_null() {
            continue;
        }
        // SAFETY: caller contract.
        let l = unsafe { &**listener };
        // data_on_readers: fire once per subscriber with a fresh data delta.
        for (subk, drs) in &dp_children[i] {
            let any = drs
                .iter()
                .any(|drk| rdelta.get(drk).map(|d| d.data).unwrap_or(false));
            if any && (*mask & STATUS_DATA_ON_READERS) != 0 {
                if let Some(cb) = l.on_data_on_readers {
                    cb(l.user_data, *subk as *mut ZeroDdsSubscriber);
                    fired += 1;
                }
            }
        }
        // inconsistent_topic: participant runtime counter delta.
        let now_ic =
            // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
            unsafe { participant_inconsistent_count(*dk as *mut ZeroDdsDomainParticipant) };
        let prev_ic = reg
            .ic_counters
            .lock()
            .ok()
            .and_then(|m| m.get(dk).copied())
            .unwrap_or(0);
        if now_ic > prev_ic && (*mask & STATUS_INCONSISTENT_TOPIC) != 0 {
            if let Some(cb) = l.on_inconsistent_topic {
                // SAFETY: first topic of the participant (or NULL).
                let tptr = unsafe {
                    let dp = &*(*dk as *mut ZeroDdsDomainParticipant);
                    dp.topics
                        .lock()
                        .ok()
                        .and_then(|g| g.first().copied())
                        .unwrap_or(core::ptr::null_mut())
                };
                cb(l.user_data, tptr);
                fired += 1;
            }
        }
        if let Ok(mut m) = reg.ic_counters.lock() {
            m.insert(*dk, now_ic);
        }
    }

    // ---- Topic level (inconsistent_topic) ----
    for (tk, listener, mask) in &topic_listeners {
        if listener.is_null() {
            continue;
        }
        // SAFETY: caller contract.
        let l = unsafe { &**listener };
        // SAFETY: topic pointer from registry; its participant pointer is valid.
        let now_ic = unsafe {
            let t = &*(*tk as *mut ZeroDdsTopic);
            participant_inconsistent_count(t.participant)
        };
        let prev_ic = reg
            .ic_counters
            .lock()
            .ok()
            .and_then(|m| m.get(tk).copied())
            .unwrap_or(0);
        if now_ic > prev_ic && (*mask & STATUS_INCONSISTENT_TOPIC) != 0 {
            if let Some(cb) = l.on_inconsistent_topic {
                cb(l.user_data, *tk as *mut ZeroDdsTopic);
                fired += 1;
            }
        }
        if let Ok(mut m) = reg.ic_counters.lock() {
            m.insert(*tk, now_ic);
        }
    }

    // ---- Update counter caches once ----
    if let Ok(mut g) = reg.dw_counters.lock() {
        for (k, now) in wnow {
            g.insert(k, now);
        }
    }
    if let Ok(mut g) = reg.dr_counters.lock() {
        for (k, now) in rnow {
            g.insert(k, now);
        }
    }

    fired
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::factory_ffi::{
        zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
    };
    use crate::participant_ffi::{
        zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
        zerodds_dp_delete_contained_entities,
    };
    use crate::publisher_ffi::zerodds_pub_create_datawriter;
    use crate::subscriber_ffi::zerodds_sub_create_datareader;
    use core::ptr;
    use core::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    // Poll-based tests share the process-global listener registry, so they
    // serialize on this lock to avoid one poll consuming another's counter
    // delta. The lock is poison-tolerant (a panicking test must not wedge
    // the rest).
    static POLL_LOCK: Mutex<()> = Mutex::new(());

    fn poll_guard() -> std::sync::MutexGuard<'static, ()> {
        POLL_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    static DATA_AVAILABLE_FIRED: AtomicUsize = AtomicUsize::new(0);
    static DATA_ON_READERS_FIRED: AtomicUsize = AtomicUsize::new(0);
    static TOPIC_INCONSISTENT_FIRED: AtomicUsize = AtomicUsize::new(0);
    static DP_INCONSISTENT_FIRED: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn cb_data_available(_u: *mut core::ffi::c_void, _dr: *mut ZeroDdsDataReader) {
        DATA_AVAILABLE_FIRED.fetch_add(1, AtomicOrdering::Relaxed);
    }
    extern "C" fn cb_data_on_readers(_u: *mut core::ffi::c_void, _s: *mut ZeroDdsSubscriber) {
        DATA_ON_READERS_FIRED.fetch_add(1, AtomicOrdering::Relaxed);
    }
    extern "C" fn cb_topic_inconsistent(_u: *mut core::ffi::c_void, _t: *mut ZeroDdsTopic) {
        TOPIC_INCONSISTENT_FIRED.fetch_add(1, AtomicOrdering::Relaxed);
    }
    extern "C" fn cb_dp_inconsistent(_u: *mut core::ffi::c_void, _t: *mut ZeroDdsTopic) {
        DP_INCONSISTENT_FIRED.fetch_add(1, AtomicOrdering::Relaxed);
    }

    #[test]
    fn dp_set_get_listener_roundtrip() {
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let p = unsafe { zerodds_dpf_create_participant(f, 81, ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert!(unsafe { zerodds_dp_get_listener(p) }.is_null());
        let l = ZeroDdsDomainParticipantListener {
            user_data: ptr::null_mut(),
            ..Default::default()
        };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dp_set_listener(p, &l, 0xFFFFFFFF) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let got = unsafe { zerodds_dp_get_listener(p) };
        assert_eq!(got as *const _, &l as *const _);
        // Clear.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dp_set_listener(p, ptr::null(), 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert!(unsafe { zerodds_dp_get_listener(p) }.is_null());
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dp_delete_contained_entities(p) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dpf_delete_participant(f, p) };
    }

    #[test]
    fn poll_listeners_returns_count_and_clears_state() {
        let _guard = poll_guard();
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let p = unsafe { zerodds_dpf_create_participant(f, 91, ptr::null()) };
        let n = c"PT";
        let tn = c"TT";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let t = unsafe { zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let pubh = unsafe { zerodds_dp_create_publisher(p, ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dw = unsafe { zerodds_pub_create_datawriter(pubh, t, ptr::null()) };
        let l = ZeroDdsDataWriterListener::default();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dw_set_listener(dw, &l, 0xFFFFFFFF) };
        // First poll round: establishes the baseline (every counter seen for
        // the first time — possibly a builtin-matching delta). Just must not
        // panic.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _fired = unsafe { zerodds_poll_listeners() };
        // Second poll round: no delta versus the first → 0 fired.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let fired2 = unsafe { zerodds_poll_listeners() };
        assert_eq!(fired2, 0, "no delta = no callbacks");
        // Clear the listener before tearing down the entity so the registry
        // never holds a dangling pointer for a later poll.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dw_set_listener(dw, ptr::null(), 0) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dp_delete_contained_entities(p) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dpf_delete_participant(f, p) };
    }

    #[test]
    fn poll_fires_data_available_and_data_on_readers() {
        let _guard = poll_guard();
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let p = unsafe { zerodds_dpf_create_participant(f, 93, ptr::null()) };
        let tn = c"DAtopic";
        let ty = c"DAtype";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let t = unsafe { zerodds_dp_create_topic(p, tn.as_ptr(), ty.as_ptr(), ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let sub = unsafe { zerodds_dp_create_subscriber(p, ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dr = unsafe { zerodds_sub_create_datareader(sub, t, ptr::null()) };

        // DataReader-level on_data_available + Subscriber-level
        // on_data_on_readers — multi-bind: both must fire for the same data.
        let dr_listener = ZeroDdsDataReaderListener {
            on_data_available: Some(cb_data_available),
            ..Default::default()
        };
        let sub_listener = ZeroDdsSubscriberListener {
            on_data_on_readers: Some(cb_data_on_readers),
            ..Default::default()
        };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dr_set_listener(dr, &dr_listener, 0xFFFFFFFF) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_sub_set_listener(sub, &sub_listener, 0xFFFFFFFF) };

        // Baseline poll (no data yet).
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_poll_listeners() };

        let da_before = DATA_AVAILABLE_FIRED.load(AtomicOrdering::Relaxed);
        let dor_before = DATA_ON_READERS_FIRED.load(AtomicOrdering::Relaxed);

        // Inject one alive sample directly into the reader slot.
        // SAFETY: dr is a live handle from create_datareader.
        let injected = unsafe {
            let drr = &*dr;
            drr.rt
                .test_inject_user_alive(drr.eid, alloc::vec::Vec::from([1u8, 2, 3]))
        };
        assert!(injected, "sample injection must hit the reader slot");

        // Next poll: data delta → both callbacks fire.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_poll_listeners() };
        assert!(
            DATA_AVAILABLE_FIRED.load(AtomicOrdering::Relaxed) > da_before,
            "on_data_available must fire on a fresh sample"
        );
        assert!(
            DATA_ON_READERS_FIRED.load(AtomicOrdering::Relaxed) > dor_before,
            "on_data_on_readers must fire (set semantics) on a fresh sample"
        );

        // Clear listeners before teardown (no dangling registry entries).
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dr_set_listener(dr, ptr::null(), 0) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_sub_set_listener(sub, ptr::null(), 0) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dp_delete_contained_entities(p) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dpf_delete_participant(f, p) };
    }

    #[test]
    fn poll_fires_inconsistent_topic_for_topic_and_participant() {
        let _guard = poll_guard();
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let p = unsafe { zerodds_dpf_create_participant(f, 94, ptr::null()) };
        let tn = c"ITtopic";
        let ty = c"ITtype";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let t = unsafe { zerodds_dp_create_topic(p, tn.as_ptr(), ty.as_ptr(), ptr::null()) };

        let topic_listener = ZeroDdsTopicListener {
            on_inconsistent_topic: Some(cb_topic_inconsistent),
            ..Default::default()
        };
        let dp_listener = ZeroDdsDomainParticipantListener {
            on_inconsistent_topic: Some(cb_dp_inconsistent),
            ..Default::default()
        };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_topic_set_listener(t, &topic_listener, 0xFFFFFFFF) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dp_set_listener(p, &dp_listener, 0xFFFFFFFF) };

        // Baseline poll.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_poll_listeners() };

        let topic_before = TOPIC_INCONSISTENT_FIRED.load(AtomicOrdering::Relaxed);
        let dp_before = DP_INCONSISTENT_FIRED.load(AtomicOrdering::Relaxed);

        // Simulate the matching path discovering a remote type mismatch.
        // SAFETY: p is a live participant handle; rt is Some while online.
        unsafe {
            if let Some(rt) = (*p).rt.as_ref() {
                rt.test_bump_inconsistent_topic();
            }
        }

        // Next poll: inconsistent-topic delta → both callbacks fire.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_poll_listeners() };
        assert!(
            TOPIC_INCONSISTENT_FIRED.load(AtomicOrdering::Relaxed) > topic_before,
            "TopicListener::on_inconsistent_topic must fire"
        );
        assert!(
            DP_INCONSISTENT_FIRED.load(AtomicOrdering::Relaxed) > dp_before,
            "DomainParticipantListener::on_inconsistent_topic must fire"
        );

        // Clear listeners before teardown.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_topic_set_listener(t, ptr::null(), 0) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dp_set_listener(p, ptr::null(), 0) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dp_delete_contained_entities(p) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dpf_delete_participant(f, p) };
    }

    #[test]
    fn dw_set_listener_clear_via_null() {
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let p = unsafe { zerodds_dpf_create_participant(f, 82, ptr::null()) };
        let n = c"T";
        let tn = c"TT";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let t = unsafe { zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let pubh = unsafe { zerodds_dp_create_publisher(p, ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dw = unsafe { zerodds_pub_create_datawriter(pubh, t, ptr::null()) };
        let l = ZeroDdsDataWriterListener {
            user_data: ptr::null_mut(),
            ..Default::default()
        };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dw_set_listener(dw, &l, 0) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert!(!unsafe { zerodds_dw_get_listener(dw) }.is_null());
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dw_set_listener(dw, ptr::null(), 0) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert!(unsafe { zerodds_dw_get_listener(dw) }.is_null());
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dp_delete_contained_entities(p) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dpf_delete_participant(f, p) };
    }
}
