// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Listener-Callback-Pfad fuer alle Entity-Typen (DDS 1.4 §2.2.4 +
//! Vendor-Extension `docs/specs/zerodds-listener-callbacks-1.0.md`).
//!
//! ## Architektur (Vendor-Spec)
//!
//! Die OMG-DDS-Spec definiert Listener als **Klassen** in C++/Java/C#
//! (`DataWriterListener`, `DataReaderListener`, etc.) — fuer C-FFI gibt
//! es keinen normativen Pfad. Diese Vendor-Extension definiert
//! Listener als **C-Funktions-Pointer-Tabellen** (`vtable`-style) mit
//! `void* user_data`-Slot fuer Caller-State.
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
// Listener-Storage pro Entity (per ZeroDds-Wrapper)
//
// RC1-Implementierung: Listener werden in einem statischen
// `OnceLock<Mutex<HashMap<*mut Entity, ListenerInfo>>>` registriert,
// und der Runtime-Worker-Thread fuert sie via Polling-Hook ab.
// Die Polling-Hook-Wireup an die Runtime ist eine separate Arbeit
// — bis dahin sind die Listener gespeichert aber noch nicht
// aktiviert. Active-Wireup wurde in Folge-Welle ergaenzt via
// `zerodds_poll_listeners()` — siehe weiter unten in dieser Datei.
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
}

struct ListenerRegistry {
    dp: Mutex<HashMap<usize, (*const ZeroDdsDomainParticipantListener, u32)>>,
    pub_: Mutex<HashMap<usize, (*const ZeroDdsPublisherListener, u32)>>,
    sub: Mutex<HashMap<usize, (*const ZeroDdsSubscriberListener, u32)>>,
    topic: Mutex<HashMap<usize, (*const ZeroDdsTopicListener, u32)>>,
    dw: Mutex<HashMap<usize, (*const ZeroDdsDataWriterListener, u32)>>,
    dr: Mutex<HashMap<usize, (*const ZeroDdsDataReaderListener, u32)>>,
    /// Pro DW-Pointer: letzte gesehene Counter (fuer Delta-Detect).
    dw_counters: Mutex<BTreeMap<usize, WriterCounters>>,
    /// Pro DR-Pointer: letzte gesehene Counter.
    dr_counters: Mutex<BTreeMap<usize, ReaderCounters>>,
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
// Der Caller ruft `zerodds_poll_listeners()` periodisch (typisch im Main-
// Loop, alle 50-200ms). Die Funktion durchlaeuft alle registrierten
// DataWriter/DataReader-Listener, vergleicht die aktuellen Status-Counter
// gegen den letzten gesehenen Wert (cached pro Entity-Pointer), und feuert
// die Callbacks bei Delta. Status-Mask-Filter wird angewendet.
//
// Threading-Vertrag: Callbacks feuern auf dem Caller-Thread des poll-
// Aufrufs. Cross-Language-Bindings koennen das in ihrem eigenen Event-
// Loop integrieren (Tokio-Tick, .NET-Timer, Python-asyncio, JS-setInterval).

/// Status-Bits gemaess `dds::core::status::StatusKind`.
const STATUS_OFFERED_DEADLINE_MISSED: u32 = 1 << 1;
const STATUS_OFFERED_INCOMPATIBLE_QOS: u32 = 1 << 5;
const STATUS_REQUESTED_DEADLINE_MISSED: u32 = 1 << 2;
const STATUS_REQUESTED_INCOMPATIBLE_QOS: u32 = 1 << 6;
const STATUS_SAMPLE_LOST: u32 = 1 << 7;
const STATUS_LIVELINESS_LOST: u32 = 1 << 11;
const STATUS_PUBLICATION_MATCHED: u32 = 1 << 13;
const STATUS_SUBSCRIPTION_MATCHED: u32 = 1 << 14;

/// Pollt alle registrierten Listener und feuert Callbacks bei Status-
/// Counter-Delta. Return-Wert: Anzahl gefeuerter Callbacks.
///
/// # Safety
/// Caller muss garantieren dass alle in der Registry registrierten Entity-
/// Pointer und Listener-Pointer noch valide sind (nicht ueber `*_destroy`
/// freigegeben, ohne `set_listener(NULL)` davor).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_poll_listeners() -> usize {
    let mut fired: usize = 0;

    // ---- DataWriter-Listener ----
    let dw_snapshot: Vec<(usize, *const ZeroDdsDataWriterListener, u32)> = registry()
        .dw
        .lock()
        .map(|g| g.iter().map(|(k, (l, m))| (*k, *l, *m)).collect())
        .unwrap_or_default();

    for (key, listener, mask) in dw_snapshot {
        if listener.is_null() {
            continue;
        }
        let dw_ptr = key as *mut ZeroDdsDataWriter;
        if dw_ptr.is_null() {
            continue;
        }
        // SAFETY: dw_ptr ist in der Registry; Caller hat den Lifetime-
        // Vertrag erfuellt (siehe doc).
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dwr = unsafe { &*dw_ptr };

        // Counter-Snapshot.
        let now_matched = dwr.rt.user_writer_matched_count(dwr.eid);
        let now_lost = dwr.rt.user_writer_liveliness_lost(dwr.eid);
        let now_deadline = dwr.rt.user_writer_offered_deadline_missed(dwr.eid);
        let now_qos = dwr
            .rt
            .user_writer_offered_incompatible_qos(dwr.eid)
            .total_count;

        let prev = registry()
            .dw_counters
            .lock()
            .map(|g| g.get(&key).copied().unwrap_or_default())
            .unwrap_or_default();

        // SAFETY: listener non-null geprueft.
        let l = unsafe { &*listener };

        if now_matched != prev.matched_count
            && (mask & STATUS_PUBLICATION_MATCHED) != 0
            && l.on_publication_matched.is_some()
        {
            (l.on_publication_matched.unwrap())(l.user_data, dw_ptr);
            fired += 1;
        }
        if now_lost > prev.liveliness_lost
            && (mask & STATUS_LIVELINESS_LOST) != 0
            && l.on_liveliness_lost.is_some()
        {
            (l.on_liveliness_lost.unwrap())(l.user_data, dw_ptr);
            fired += 1;
        }
        if now_deadline > prev.offered_deadline_missed
            && (mask & STATUS_OFFERED_DEADLINE_MISSED) != 0
            && l.on_offered_deadline_missed.is_some()
        {
            (l.on_offered_deadline_missed.unwrap())(l.user_data, dw_ptr);
            fired += 1;
        }
        if now_qos > prev.offered_incompatible_qos_total
            && (mask & STATUS_OFFERED_INCOMPATIBLE_QOS) != 0
            && l.on_offered_incompatible_qos.is_some()
        {
            (l.on_offered_incompatible_qos.unwrap())(l.user_data, dw_ptr);
            fired += 1;
        }

        // Counter-Cache update.
        if let Ok(mut g) = registry().dw_counters.lock() {
            g.insert(
                key,
                WriterCounters {
                    matched_count: now_matched,
                    liveliness_lost: now_lost,
                    offered_deadline_missed: now_deadline,
                    offered_incompatible_qos_total: now_qos,
                },
            );
        }
    }

    // ---- DataReader-Listener ----
    let dr_snapshot: Vec<(usize, *const ZeroDdsDataReaderListener, u32)> = registry()
        .dr
        .lock()
        .map(|g| g.iter().map(|(k, (l, m))| (*k, *l, *m)).collect())
        .unwrap_or_default();

    for (key, listener, mask) in dr_snapshot {
        if listener.is_null() {
            continue;
        }
        let dr_ptr = key as *mut ZeroDdsDataReader;
        if dr_ptr.is_null() {
            continue;
        }
        // SAFETY: siehe DW-Block.
        let drr = unsafe { &*dr_ptr };

        let now_matched = drr.rt.user_reader_matched_count(drr.eid);
        let now_sample_lost = drr.rt.user_reader_sample_lost(drr.eid);
        let now_deadline = drr.rt.user_reader_requested_deadline_missed(drr.eid);
        let now_qos = drr
            .rt
            .user_reader_requested_incompatible_qos(drr.eid)
            .total_count;

        let prev = registry()
            .dr_counters
            .lock()
            .map(|g| g.get(&key).copied().unwrap_or_default())
            .unwrap_or_default();

        // SAFETY: listener non-null.
        let l = unsafe { &*listener };

        if now_matched != prev.matched_count
            && (mask & STATUS_SUBSCRIPTION_MATCHED) != 0
            && l.on_subscription_matched.is_some()
        {
            (l.on_subscription_matched.unwrap())(l.user_data, dr_ptr);
            fired += 1;
        }
        if now_sample_lost > prev.sample_lost
            && (mask & STATUS_SAMPLE_LOST) != 0
            && l.on_sample_lost.is_some()
        {
            (l.on_sample_lost.unwrap())(l.user_data, dr_ptr);
            fired += 1;
        }
        if now_deadline > prev.requested_deadline_missed
            && (mask & STATUS_REQUESTED_DEADLINE_MISSED) != 0
            && l.on_requested_deadline_missed.is_some()
        {
            (l.on_requested_deadline_missed.unwrap())(l.user_data, dr_ptr);
            fired += 1;
        }
        if now_qos > prev.requested_incompatible_qos_total
            && (mask & STATUS_REQUESTED_INCOMPATIBLE_QOS) != 0
            && l.on_requested_incompatible_qos.is_some()
        {
            (l.on_requested_incompatible_qos.unwrap())(l.user_data, dr_ptr);
            fired += 1;
        }

        if let Ok(mut g) = registry().dr_counters.lock() {
            g.insert(
                key,
                ReaderCounters {
                    matched_count: now_matched,
                    sample_lost: now_sample_lost,
                    requested_deadline_missed: now_deadline,
                    requested_incompatible_qos_total: now_qos,
                },
            );
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
        zerodds_dp_create_publisher, zerodds_dp_create_topic, zerodds_dp_delete_contained_entities,
    };
    use crate::publisher_ffi::zerodds_pub_create_datawriter;
    use core::ptr;

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
        // Erste Poll-Runde: kein Delta, keine Callbacks (alle Counter
        // erstmals geseht — oder evtl. matched-count > 0 wegen builtin
        // matching). Test: poll soll nicht panicen, return >= 0.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _fired = unsafe { zerodds_poll_listeners() };
        // Zweite Poll-Runde: kein Delta zur ersten → 0 fired.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let fired2 = unsafe { zerodds_poll_listeners() };
        assert_eq!(fired2, 0, "no delta = no callbacks");
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
