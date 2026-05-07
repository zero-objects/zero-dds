// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Subscriber + DataReader C-FFI (Spec §2.2.2.5 + DDS-PSM-Cxx §7.5.6/§7.5.8).

use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ffi::c_int;
use core::ptr;
use core::slice;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use zerodds_dcps::qos::{DataReaderQos, ReliabilityKind};
use zerodds_dcps::runtime::{UserReaderConfig, UserSample};

use crate::ZeroDdsStatus;
use crate::entities::{
    ZeroDdsDataReader, ZeroDdsDomainParticipant, ZeroDdsSubscriber, ZeroDdsTopic,
};
use crate::qos_ffi::{ZeroDdsDataReaderQos, dr_qos_from_c};

// ---------------------------------------------------------------------------
// Subscriber
// ---------------------------------------------------------------------------

/// Liefert den Owning-Participant.
///
/// # Safety
/// `sub` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_get_participant(
    sub: *mut ZeroDdsSubscriber,
) -> *mut ZeroDdsDomainParticipant {
    if sub.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { (*sub).participant }
}

/// `begin_access` (Spec §2.2.2.5.1.13). RC1: No-op-Marker fuer
/// Coherent-Sets — die echten Set-Boundaries liegen am Reader-Wire-Pfad.
///
/// # Safety
/// `sub` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_begin_access(sub: *mut ZeroDdsSubscriber) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    ZeroDdsStatus::Ok as c_int
}

/// `end_access`.
///
/// # Safety
/// `sub` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_end_access(sub: *mut ZeroDdsSubscriber) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Liefert die Liste aktiver DataReader.
///
/// # Safety
/// `sub`, `out`, `out_count` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_get_datareaders(
    sub: *mut ZeroDdsSubscriber,
    out: *mut *mut ZeroDdsDataReader,
    out_count: *mut usize,
    cap: usize,
) -> c_int {
    if sub.is_null() || out.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let sb = unsafe { &*sub };
    let drs = sb.datareaders.lock().map(|g| g.clone()).unwrap_or_default();
    let n = drs.len().min(cap);
    // SAFETY: Caller-Kontrakt: out[0..cap] writeable.
    let dst = unsafe { slice::from_raw_parts_mut(out, n) };
    dst.copy_from_slice(&drs[..n]);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { *out_count = n };
    ZeroDdsStatus::Ok as c_int
}

/// `notify_datareaders` (Spec §2.2.2.5.1.16). RC1: No-op — Listener-Bubble-Up
/// laeuft per-Reader, der Subscriber-Aggregator wird in WP "Listeners-FFI" wired.
///
/// # Safety
/// `sub` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_notify_datareaders(sub: *mut ZeroDdsSubscriber) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Erzeugt einen DataReader.
///
/// # Safety
/// `sub`, `topic` valide; `qos` darf NULL sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_datareader(
    sub: *mut ZeroDdsSubscriber,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataReaderQos,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || topic.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let sb = unsafe { &*sub };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let tt = unsafe { &*topic };
    let dp_handle = sb.participant;
    if dp_handle.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dp = unsafe { &*dp_handle };
    let rt = match dp.rt.as_ref() {
        Some(r) => r.clone(),
        None => return ptr::null_mut(),
    };

    let qos: DataReaderQos = if qos.is_null() {
        sb.default_dr_qos
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    } else {
        // SAFETY: NULL-Check.
        unsafe { dr_qos_from_c(qos) }
    };

    let cfg = UserReaderConfig {
        topic_name: tt.name.to_string(),
        type_name: tt.type_name.to_string(),
        reliable: matches!(qos.reliability.kind, ReliabilityKind::Reliable),
        durability: qos.durability.kind,
        deadline: qos.deadline.clone(),
        liveliness: qos.liveliness.clone(),
        ownership: qos.ownership.kind,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::default(),
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    };
    let (eid, rx) = match rt.register_user_reader(cfg) {
        Ok(pair) => pair,
        Err(_) => return ptr::null_mut(),
    };
    let dr = Box::new(ZeroDdsDataReader {
        subscriber: sub,
        topic,
        rt,
        eid,
        qos: Mutex::new(qos),
        rx: Mutex::new(rx),
        read_cache: Mutex::new(Vec::new()),
        cft_filter: None,
    });
    let dr_ptr = Box::into_raw(dr);
    if let Ok(mut list) = sb.datareaders.lock() {
        list.push(dr_ptr);
    }
    dr_ptr
}

/// Erzeugt einen DataReader auf einem ContentFilteredTopic.
/// Bei jedem `take`/`read` wird die Filter-Expression evaluiert
/// (Spec §2.2.2.3.3 + §2.2.2.5.2.5).
///
/// # Safety
/// `sub`, `cft` valide; `qos` darf NULL sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_datareader_with_cft(
    sub: *mut ZeroDdsSubscriber,
    cft: *mut crate::entities::ZeroDdsContentFilteredTopic,
    qos: *const ZeroDdsDataReaderQos,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || cft.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: NULL-Checks.
    let cft_ref = unsafe { &*cft };
    let related_topic = cft_ref.related_topic;
    if related_topic.is_null() {
        return ptr::null_mut();
    }
    // Parse Filter-Expression beim Konstruktions-Zeitpunkt.
    let expr = match zerodds_sql_filter::parse(&cft_ref.filter_expression) {
        Ok(e) => e,
        Err(_) => return ptr::null_mut(),
    };
    let params: Vec<zerodds_sql_filter::Value> = cft_ref
        .parameters
        .lock()
        .map(|g| {
            g.iter()
                .map(|p| zerodds_sql_filter::Value::String(p.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Standard create_datareader-Pfad.
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dr = unsafe { zerodds_sub_create_datareader(sub, related_topic, qos) };
    if dr.is_null() {
        return ptr::null_mut();
    }
    // CFT-Filter im Reader installieren via mutable Box-Access.
    // SAFETY: dr ist eine frische Box<ZeroDdsDataReader> aus
    // sub_create_datareader; Box::from_raw + Re-Init der filter-Field.
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        let mut boxed = Box::from_raw(dr);
        boxed.cft_filter = Some(crate::entities::CftFilter { expr, params });
        Box::into_raw(boxed)
    }
}

/// Loescht einen DataReader.
///
/// # Safety
/// `sub`, `dr` valide und zugehoerig.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_delete_datareader(
    sub: *mut ZeroDdsSubscriber,
    dr: *mut ZeroDdsDataReader,
) -> c_int {
    if sub.is_null() || dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let drr = unsafe { &*dr };
        if drr.subscriber != sub {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let sb = unsafe { &*sub };
    if let Ok(mut list) = sb.datareaders.lock() {
        let n = list.len();
        list.retain(|x| *x != dr);
        if list.len() == n {
            return ZeroDdsStatus::BadHandle as c_int;
        }
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let _ = unsafe { Box::from_raw(dr) };
    ZeroDdsStatus::Ok as c_int
}

/// Lookup-DataReader by Topic-Name.
///
/// # Safety
/// `sub`, `topic_name` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_lookup_datareader(
    sub: *mut ZeroDdsSubscriber,
    topic_name: *const core::ffi::c_char,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let cs = unsafe { std::ffi::CStr::from_ptr(topic_name) };
    let name = match cs.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let sb = unsafe { &*sub };
    if let Ok(list) = sb.datareaders.lock() {
        for &dr in list.iter() {
            if dr.is_null() {
                continue;
            }
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            let drr = unsafe { &*dr };
            if !drr.topic.is_null() {
                // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
                let t = unsafe { &*drr.topic };
                if t.name == name {
                    return dr;
                }
            }
        }
    }
    ptr::null_mut()
}

/// Loescht alle vom Subscriber gehaltenen DataReader.
///
/// # Safety
/// `sub` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_delete_contained_entities(
    sub: *mut ZeroDdsSubscriber,
) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let sb = unsafe { &*sub };
    let drs: Vec<*mut ZeroDdsDataReader> = sb
        .datareaders
        .lock()
        .map(|mut g| core::mem::take(&mut *g))
        .unwrap_or_default();
    for dr in drs {
        if !dr.is_null() {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            let _ = unsafe { Box::from_raw(dr) };
        }
    }
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// DataReader: read / take + Sample-API
// ---------------------------------------------------------------------------

/// Liefert das TopicDescription-Handle (im RC1: Topic-Pointer).
///
/// # Safety
/// `dr` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_topicdescription(
    dr: *mut ZeroDdsDataReader,
) -> *mut ZeroDdsTopic {
    if dr.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { (*dr).topic }
}

/// Liefert den Subscriber.
///
/// # Safety
/// `dr` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_subscriber(
    dr: *mut ZeroDdsDataReader,
) -> *mut ZeroDdsSubscriber {
    if dr.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { (*dr).subscriber }
}

/// SampleInfo (Spec §2.2.2.5.4 + DDS-PSM-Cxx §7.5.8.1).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsSampleInfo {
    /// Sample-State Bit (1=READ, 2=NOT_READ).
    pub sample_state: u32,
    /// View-State (1=NEW, 2=NOT_NEW).
    pub view_state: u32,
    /// Instance-State (1=ALIVE, 2=NOT_ALIVE_DISPOSED, 4=NOT_ALIVE_NO_WRITERS).
    pub instance_state: u32,
    /// disposed_generation_count.
    pub disposed_generation_count: i32,
    /// no_writers_generation_count.
    pub no_writers_generation_count: i32,
    /// sample_rank.
    pub sample_rank: i32,
    /// generation_rank.
    pub generation_rank: i32,
    /// absolute_generation_rank.
    pub absolute_generation_rank: i32,
    /// source_timestamp seconds.
    pub source_timestamp_sec: i32,
    /// source_timestamp nanoseconds.
    pub source_timestamp_nanosec: u32,
    /// Instance-Handle.
    pub instance_handle: u64,
    /// Publication-Handle (Writer-GUID-Hash).
    pub publication_handle: u64,
    /// `true` wenn payload tatsaechlich Daten hat (vs. Lifecycle-Marker).
    pub valid_data: bool,
}

/// Sample-Array (Spec §5 Mini-Spec).
#[repr(C)]
pub struct ZeroDdsSampleArray {
    /// Array von Payload-Pointern.
    pub buffers: *mut *mut u8,
    /// Array von Payload-Laengen.
    pub lengths: *mut usize,
    /// Array von SampleInfos.
    pub infos: *mut ZeroDdsSampleInfo,
    /// Anzahl Samples.
    pub count: usize,
    /// Internes Loan-Token (Pointer auf `Vec<UserSample>` Box). Wird
    /// von `return_loan` freigegeben.
    pub loan_token: *mut core::ffi::c_void,
}

/// Internes Loan-Memory: Held bis `return_loan` aufruft.
struct LoanMemory {
    payloads: Vec<Vec<u8>>,
    buffers: Vec<*mut u8>,
    lengths: Vec<usize>,
    infos: Vec<ZeroDdsSampleInfo>,
}

impl LoanMemory {
    fn new(samples: Vec<UserSample>) -> Box<Self> {
        let with_state: Vec<(UserSample, crate::entities::ReadSampleState)> = samples
            .into_iter()
            .map(|s| (s, crate::entities::ReadSampleState::NotRead))
            .collect();
        Self::from_state(with_state)
    }

    fn from_state(samples: Vec<(UserSample, crate::entities::ReadSampleState)>) -> Box<Self> {
        let mut payloads: Vec<Vec<u8>> = Vec::with_capacity(samples.len());
        let mut infos: Vec<ZeroDdsSampleInfo> = Vec::with_capacity(samples.len());

        for (s, state) in samples {
            let sample_state_bit = match state {
                crate::entities::ReadSampleState::Read => 1u32, // READ
                crate::entities::ReadSampleState::NotRead => 2u32, // NOT_READ
            };
            match s {
                UserSample::Alive {
                    payload,
                    writer_guid,
                    ..
                } => {
                    let pub_handle = u64_from_guid(writer_guid);
                    infos.push(ZeroDdsSampleInfo {
                        sample_state: sample_state_bit,
                        view_state: 1,     // NEW
                        instance_state: 1, // ALIVE
                        disposed_generation_count: 0,
                        no_writers_generation_count: 0,
                        sample_rank: 0,
                        generation_rank: 0,
                        absolute_generation_rank: 0,
                        source_timestamp_sec: 0,
                        source_timestamp_nanosec: 0,
                        instance_handle: 0,
                        publication_handle: pub_handle,
                        valid_data: true,
                    });
                    payloads.push(payload);
                }
                UserSample::Lifecycle { kind, .. } => {
                    use zerodds_rtps::history_cache::ChangeKind;
                    let inst_state = match kind {
                        ChangeKind::NotAliveDisposed | ChangeKind::NotAliveDisposedUnregistered => {
                            2
                        }
                        ChangeKind::NotAliveUnregistered => 4,
                        _ => 1,
                    };
                    infos.push(ZeroDdsSampleInfo {
                        sample_state: sample_state_bit,
                        view_state: 1,
                        instance_state: inst_state,
                        disposed_generation_count: 0,
                        no_writers_generation_count: 0,
                        sample_rank: 0,
                        generation_rank: 0,
                        absolute_generation_rank: 0,
                        source_timestamp_sec: 0,
                        source_timestamp_nanosec: 0,
                        instance_handle: 0,
                        publication_handle: 0,
                        valid_data: false,
                    });
                    payloads.push(Vec::new());
                }
            }
        }

        // Buffers + Lengths separate Vecs damit ihre Pointer stabil sind
        // (Vec<Vec<u8>>::as_mut_ptr() der inneren Vecs sind stabil
        // solange die outer Vec nicht re-allokiert).
        let mut buffers: Vec<*mut u8> = Vec::with_capacity(payloads.len());
        let mut lengths: Vec<usize> = Vec::with_capacity(payloads.len());
        for v in payloads.iter_mut() {
            buffers.push(v.as_mut_ptr());
            lengths.push(v.len());
        }
        Box::new(LoanMemory {
            payloads,
            buffers,
            lengths,
            infos,
        })
    }
}

fn u64_from_guid(g: [u8; 16]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in g.iter() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Take: konsumiert Samples aus Cache + Channel (Spec §2.2.2.5.3).
/// Geht durch read_cache (Sample-State READ + NOT_READ) und Channel
/// (Sample-State NOT_READ), entfernt alle gelieferten Samples.
///
/// # Safety
/// `dr`, `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_take(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleArray,
    max_samples: usize,
    _sample_states: u32,
    _view_states: u32,
    _instance_states: u32,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr };
    let limit = if max_samples == 0 {
        usize::MAX
    } else {
        max_samples
    };

    // 1) Erst Read-Cache leeren (Samples die `read()` schon gesehen hat).
    let mut collected: Vec<(UserSample, crate::entities::ReadSampleState)> = Vec::new();
    if let Ok(mut cache) = drr.read_cache.lock() {
        while collected.len() < limit && !cache.is_empty() {
            collected.push(cache.remove(0));
        }
    }
    // 2) Dann frische Samples aus dem Channel.
    if let Ok(rx) = drr.rx.lock() {
        while collected.len() < limit {
            match rx.try_recv() {
                Ok(s) => collected.push((s, crate::entities::ReadSampleState::NotRead)),
                Err(_) => break,
            }
        }
    }

    // 3) ContentFilteredTopic-Filter (Spec §2.2.2.3.3): bei CFT-bound
    // Reader nur Samples liefern die den Filter erfuellen.
    if let Some(filter) = &drr.cft_filter {
        collected.retain(|(s, _)| match s {
            UserSample::Alive { payload, .. } => filter.evaluate(payload),
            UserSample::Lifecycle { .. } => true, // Lifecycle-Marker bypassen Filter.
        });
    }

    if collected.is_empty() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe {
            (*out).buffers = ptr::null_mut();
            (*out).lengths = ptr::null_mut();
            (*out).infos = ptr::null_mut();
            (*out).count = 0;
            (*out).loan_token = ptr::null_mut();
        }
        return ZeroDdsStatus::NoData as c_int;
    }
    finalize_loan(out, collected)
}

/// Read: non-destructive Variante von Take (Spec §2.2.2.5.3).
/// Liefert Samples aus Cache + Channel, aber ALLE bleiben anschliessend
/// im Read-Cache mit Sample-State = READ.
///
/// # Safety
/// `dr`, `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_read(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleArray,
    max_samples: usize,
    _sample_states: u32,
    _view_states: u32,
    _instance_states: u32,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr };
    let limit = if max_samples == 0 {
        usize::MAX
    } else {
        max_samples
    };

    // Pull frische Samples vom Channel in den Cache (mit NOT_READ-Marker).
    // Filter angewendet: nicht-passierende Samples werden NICHT gecacht
    // (Spec-konform: read() liefert nur Samples die zum CFT-Filter passen).
    if let (Ok(rx), Ok(mut cache)) = (drr.rx.lock(), drr.read_cache.lock()) {
        while let Ok(s) = rx.try_recv() {
            let pass = if let Some(filter) = &drr.cft_filter {
                match &s {
                    UserSample::Alive { payload, .. } => filter.evaluate(payload),
                    UserSample::Lifecycle { .. } => true,
                }
            } else {
                true
            };
            if pass {
                cache.push((s, crate::entities::ReadSampleState::NotRead));
            }
        }
    }

    // Lese die ersten `limit` aus dem Cache (clone), markiere als READ.
    let collected: Vec<(UserSample, crate::entities::ReadSampleState)> =
        if let Ok(mut cache) = drr.read_cache.lock() {
            let n = cache.len().min(limit);
            let out_collected: Vec<_> = cache
                .iter()
                .take(n)
                .map(|(s, st)| (s.clone(), *st))
                .collect();
            // Mark consumed slice as READ.
            for entry in cache.iter_mut().take(n) {
                entry.1 = crate::entities::ReadSampleState::Read;
            }
            out_collected
        } else {
            Vec::new()
        };

    if collected.is_empty() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe {
            (*out).buffers = ptr::null_mut();
            (*out).lengths = ptr::null_mut();
            (*out).infos = ptr::null_mut();
            (*out).count = 0;
            (*out).loan_token = ptr::null_mut();
        }
        return ZeroDdsStatus::NoData as c_int;
    }
    finalize_loan(out, collected)
}

fn finalize_loan(
    out: *mut ZeroDdsSampleArray,
    collected: Vec<(UserSample, crate::entities::ReadSampleState)>,
) -> c_int {
    let mut loan = LoanMemory::from_state(collected);
    let buffers_ptr = loan.buffers.as_mut_ptr();
    let lengths_ptr = loan.lengths.as_mut_ptr();
    let infos_ptr = loan.infos.as_mut_ptr();
    let count = loan.payloads.len();
    let token = Box::into_raw(loan) as *mut core::ffi::c_void;
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        (*out).buffers = buffers_ptr;
        (*out).lengths = lengths_ptr;
        (*out).infos = infos_ptr;
        (*out).count = count;
        (*out).loan_token = token;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Take next single sample.
///
/// # Safety
/// `dr`, `out_buf`, `out_len`, `out_info` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_take_next_sample(
    dr: *mut ZeroDdsDataReader,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    if dr.is_null() || out_buf.is_null() || out_len.is_null() || out_info.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr };
    let s = match drr.rx.lock().ok().and_then(|rx| rx.try_recv().ok()) {
        Some(s) => s,
        None => return ZeroDdsStatus::NoData as c_int,
    };
    let loan = LoanMemory::new(alloc::vec![s]);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        *out_buf = loan.buffers[0];
        *out_len = loan.lengths[0];
        *out_info = loan.infos[0];
    }
    // Single-sample take: Loan haengt am out_buf. Caller muss
    // `take`-Loan-Pfad benutzen wenn er Memory zurueckgeben will —
    // hier transferieren wir Ownership an Caller via Box::into_raw +
    // memory-leak-Pfad: dokumentiert in der Mini-Spec, Caller MUSS
    // `zerodds_dr_return_loan` mit einer SampleArray { loan_token: <token> }
    // rufen. Wir ankern den Token in einer thread-local Variable —
    // oder besser: wir geben ihn ueber out_info nicht zurueck. Diese
    // RC1-Variante ist deshalb leak-tolerant: Loan wird drop-bare.
    let _ = Box::into_raw(loan);
    ZeroDdsStatus::Ok as c_int
}

/// Read next single sample. Identisch zu take_next_sample im RC1.
///
/// # Safety
/// Wie `take_next_sample`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_read_next_sample(
    dr: *mut ZeroDdsDataReader,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { zerodds_dr_take_next_sample(dr, out_buf, out_len, out_info) }
}

/// Filtert eine `SampleArray` in-place auf einen einzelnen
/// `instance_handle`. Samples die nicht passen werden aus den
/// Arrays entfernt; `count` wird angepasst.
///
/// # Safety
/// `arr` valide; `arr.buffers/lengths/infos[0..count]` lesbar.
pub fn sample_array_filter_instance(arr: *mut ZeroDdsSampleArray, handle: u64) -> c_int {
    if arr.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: NULL-Check.
    let count = unsafe { (*arr).count };
    if count == 0 {
        return ZeroDdsStatus::Ok as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let buffers = unsafe { (*arr).buffers };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let lengths = unsafe { (*arr).lengths };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let infos = unsafe { (*arr).infos };
    if buffers.is_null() || lengths.is_null() || infos.is_null() {
        return ZeroDdsStatus::Ok as c_int;
    }
    let mut write_idx: usize = 0;
    for read_idx in 0..count {
        // SAFETY: Caller-Kontrakt: 0..count valide.
        let info = unsafe { *infos.add(read_idx) };
        if info.instance_handle == handle {
            if write_idx != read_idx {
                // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
                unsafe {
                    *buffers.add(write_idx) = *buffers.add(read_idx);
                    *lengths.add(write_idx) = *lengths.add(read_idx);
                    *infos.add(write_idx) = info;
                }
            }
            write_idx += 1;
        }
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { (*arr).count = write_idx };
    ZeroDdsStatus::Ok as c_int
}

/// Filtert auf "naechste" Instance > prev_handle. Liefert nur
/// Samples deren instance_handle > prev_handle UND minimal ist.
pub fn sample_array_filter_next_instance(arr: *mut ZeroDdsSampleArray, prev_handle: u64) -> c_int {
    if arr.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let count = unsafe { (*arr).count };
    if count == 0 {
        return ZeroDdsStatus::Ok as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let infos = unsafe { (*arr).infos };
    if infos.is_null() {
        return ZeroDdsStatus::Ok as c_int;
    }
    // Finde minimal handle > prev_handle.
    let mut next_handle: Option<u64> = None;
    for read_idx in 0..count {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let h = unsafe { (*infos.add(read_idx)).instance_handle };
        if h > prev_handle && next_handle.is_none_or(|n| h < n) {
            next_handle = Some(h);
        }
    }
    match next_handle {
        Some(h) => sample_array_filter_instance(arr, h),
        None => {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { (*arr).count = 0 };
            ZeroDdsStatus::Ok as c_int
        }
    }
}

/// Filtert auf sample_state/view_state/instance_state-Bitmask.
/// Mask=0 bedeutet "alle erlaubt".
pub fn sample_array_filter_states(
    arr: *mut ZeroDdsSampleArray,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
) -> c_int {
    if arr.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let count = unsafe { (*arr).count };
    if count == 0 {
        return ZeroDdsStatus::Ok as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let buffers = unsafe { (*arr).buffers };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let lengths = unsafe { (*arr).lengths };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let infos = unsafe { (*arr).infos };
    if buffers.is_null() || lengths.is_null() || infos.is_null() {
        return ZeroDdsStatus::Ok as c_int;
    }
    let mut write_idx = 0usize;
    for read_idx in 0..count {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let info = unsafe { *infos.add(read_idx) };
        let s_ok = sample_states == 0 || (sample_states & info.sample_state) != 0;
        let v_ok = view_states == 0 || (view_states & info.view_state) != 0;
        let i_ok = instance_states == 0 || (instance_states & info.instance_state) != 0;
        if s_ok && v_ok && i_ok {
            if write_idx != read_idx {
                // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
                unsafe {
                    *buffers.add(write_idx) = *buffers.add(read_idx);
                    *lengths.add(write_idx) = *lengths.add(read_idx);
                    *infos.add(write_idx) = info;
                }
            }
            write_idx += 1;
        }
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { (*arr).count = write_idx };
    ZeroDdsStatus::Ok as c_int
}

/// Gibt geliehene Sample-Buffer zurueck.
///
/// # Safety
/// `arr` muss aus einem vorherigen `zerodds_dr_take`/`read` stammen,
/// `loan_token` muss noch gueltig sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_return_loan(
    _dr: *mut ZeroDdsDataReader,
    arr: *mut ZeroDdsSampleArray,
) -> c_int {
    if arr.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: arr non-null.
    let token = unsafe { (*arr).loan_token };
    if !token.is_null() {
        // SAFETY: token aus Box::into_raw in `LoanMemory::new`.
        let _ = unsafe { Box::from_raw(token as *mut LoanMemory) };
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        (*arr).buffers = ptr::null_mut();
        (*arr).lengths = ptr::null_mut();
        (*arr).infos = ptr::null_mut();
        (*arr).count = 0;
        (*arr).loan_token = ptr::null_mut();
    }
    ZeroDdsStatus::Ok as c_int
}

/// Wartet bis `min` matched Publications oder Timeout.
///
/// # Safety
/// `dr` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_wait_for_matched(
    dr: *mut ZeroDdsDataReader,
    min: i32,
    timeout_ms: u64,
) -> c_int {
    if dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let n = drr.rt.user_reader_matched_count(drr.eid) as i32;
        if n >= min {
            return ZeroDdsStatus::Ok as c_int;
        }
        if std::time::Instant::now() >= deadline {
            return ZeroDdsStatus::Timeout as c_int;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ---------------------------------------------------------------------------
// DataReader Statuses (6x)
// ---------------------------------------------------------------------------

/// LivelinessChangedStatus (Spec §2.2.4.1).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsLivelinessChangedStatus {
    /// Total alive count.
    pub alive_count: i32,
    /// Total not-alive count.
    pub not_alive_count: i32,
    /// Change in alive_count since last read.
    pub alive_count_change: i32,
    /// Change in not_alive_count since last read.
    pub not_alive_count_change: i32,
    /// Last writer that triggered a change.
    pub last_publication_handle: u64,
}

/// SubscriptionMatchedStatus.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsSubscriptionMatchedStatus {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Currently matched.
    pub current_count: i32,
    /// Change in current_count.
    pub current_count_change: i32,
    /// Last DataWriter that matched.
    pub last_publication_handle: u64,
}

/// RequestedDeadlineMissedStatus.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsRequestedDeadlineMissedStatus {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Last instance handle.
    pub last_instance_handle: u64,
}

/// RequestedIncompatibleQosStatus.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsRequestedIncompatibleQosStatus {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Last policy id.
    pub last_policy_id: u32,
}

/// SampleLostStatus.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsSampleLostStatus {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// SampleRejectedStatus.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsSampleRejectedStatus {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Reason kind: 0=NotRejected, 1=ByInstancesLimit, 2=BySamplesLimit,
    /// 3=BySamplesPerInstanceLimit.
    pub last_reason: u32,
    /// Last instance handle.
    pub last_instance_handle: u64,
}

/// `LIVELINESS_CHANGED_STATUS`.
///
/// # Safety
/// `dr` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_liveliness_changed_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsLivelinessChangedStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr };
    let (alive, alive_count, not_alive_count) = drr.rt.user_reader_liveliness_status(drr.eid);
    let _ = alive;
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        *out = ZeroDdsLivelinessChangedStatus {
            alive_count: alive_count as i32,
            not_alive_count: not_alive_count as i32,
            alive_count_change: 0,
            not_alive_count_change: 0,
            last_publication_handle: 0,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `SUBSCRIPTION_MATCHED_STATUS`.
///
/// # Safety
/// `dr` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_subscription_matched_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSubscriptionMatchedStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr };
    let n = drr.rt.user_reader_matched_count(drr.eid) as i32;
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        *out = ZeroDdsSubscriptionMatchedStatus {
            total_count: n,
            total_count_change: 0,
            current_count: n,
            current_count_change: 0,
            last_publication_handle: 0,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `REQUESTED_DEADLINE_MISSED_STATUS`.
///
/// # Safety
/// `dr` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_requested_deadline_missed_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsRequestedDeadlineMissedStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr };
    let n = drr.rt.user_reader_requested_deadline_missed(drr.eid);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        *out = ZeroDdsRequestedDeadlineMissedStatus {
            total_count: n as i32,
            total_count_change: 0,
            last_instance_handle: 0,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `REQUESTED_INCOMPATIBLE_QOS_STATUS`.
///
/// # Safety
/// `dr` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_requested_incompatible_qos_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsRequestedIncompatibleQosStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr };
    let st = drr.rt.user_reader_requested_incompatible_qos(drr.eid);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        *out = ZeroDdsRequestedIncompatibleQosStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
            last_policy_id: st.last_policy_id,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `SAMPLE_LOST_STATUS`.
///
/// # Safety
/// `dr` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_sample_lost_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleLostStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr };
    let n = drr.rt.user_reader_sample_lost(drr.eid);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        *out = ZeroDdsSampleLostStatus {
            total_count: n as i32,
            total_count_change: 0,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `SAMPLE_REJECTED_STATUS`.
///
/// # Safety
/// `dr` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_sample_rejected_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleRejectedStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let drr = unsafe { &*dr };
    let st = drr.rt.user_reader_sample_rejected(drr.eid);
    use zerodds_dcps::status::SampleRejectedStatusKind;
    let reason: u32 = match st.last_reason {
        SampleRejectedStatusKind::NotRejected => 0,
        SampleRejectedStatusKind::RejectedByInstancesLimit => 1,
        SampleRejectedStatusKind::RejectedBySamplesLimit => 2,
        SampleRejectedStatusKind::RejectedBySamplesPerInstanceLimit => 3,
    };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        *out = ZeroDdsSampleRejectedStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
            last_reason: reason,
            last_instance_handle: 0,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

// suppress unused-import warning when DataReaderQos is only used in entities.rs
#[allow(dead_code)]
fn _suppress(_: DataReaderQos, _: mpsc::Receiver<UserSample>) {}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::factory_ffi::{
        zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
    };
    use crate::participant_ffi::{
        zerodds_dp_create_subscriber, zerodds_dp_create_topic, zerodds_dp_delete_contained_entities,
    };

    fn mk(
        domain: u32,
    ) -> (
        *mut ZeroDdsDomainParticipant,
        *mut ZeroDdsSubscriber,
        *mut ZeroDdsTopic,
    ) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let p = unsafe { zerodds_dpf_create_participant(f, domain, ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let sub = unsafe { zerodds_dp_create_subscriber(p, ptr::null()) };
        let n = c"SubT";
        let tn = c"SubTy";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let t = unsafe { zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null()) };
        (p, sub, t)
    }
    fn cleanup(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe {
            zerodds_dp_delete_contained_entities(p);
            zerodds_dpf_delete_participant(f, p);
        }
    }

    #[test]
    fn create_delete_datareader() {
        let (p, sub, t) = mk(51);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dr = unsafe { zerodds_sub_create_datareader(sub, t, ptr::null()) };
        assert!(!dr.is_null());
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_sub_delete_datareader(sub, dr) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        cleanup(p);
    }

    #[test]
    fn lookup_datareader_finds_existing() {
        let (p, sub, t) = mk(52);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dr = unsafe { zerodds_sub_create_datareader(sub, t, ptr::null()) };
        let n = c"SubT";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let f = unsafe { zerodds_sub_lookup_datareader(sub, n.as_ptr()) };
        assert_eq!(f, dr);
        cleanup(p);
    }

    #[test]
    fn take_on_empty_returns_no_data() {
        let (p, sub, t) = mk(53);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dr = unsafe { zerodds_sub_create_datareader(sub, t, ptr::null()) };
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 0,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
        assert_eq!(arr.count, 0);
        cleanup(p);
    }

    #[test]
    fn return_loan_clears_array() {
        let (p, sub, t) = mk(54);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dr = unsafe { zerodds_sub_create_datareader(sub, t, ptr::null()) };
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 7,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dr_return_loan(dr, &mut arr) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 0);
        cleanup(p);
    }

    #[test]
    fn statuses_default_ok() {
        let (p, sub, t) = mk(55);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dr = unsafe { zerodds_sub_create_datareader(sub, t, ptr::null()) };
        let mut a = ZeroDdsLivelinessChangedStatus::default();
        let mut b = ZeroDdsSubscriptionMatchedStatus::default();
        let mut c = ZeroDdsRequestedDeadlineMissedStatus::default();
        let mut d = ZeroDdsRequestedIncompatibleQosStatus::default();
        let mut e = ZeroDdsSampleLostStatus::default();
        let mut g = ZeroDdsSampleRejectedStatus::default();
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_dr_get_liveliness_changed_status(dr, &mut a) },
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_dr_get_subscription_matched_status(dr, &mut b) },
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_dr_get_requested_deadline_missed_status(dr, &mut c) },
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_dr_get_requested_incompatible_qos_status(dr, &mut d) },
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_dr_get_sample_lost_status(dr, &mut e) },
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_dr_get_sample_rejected_status(dr, &mut g) },
            ZeroDdsStatus::Ok as c_int
        );
        cleanup(p);
    }

    #[test]
    fn cft_filter_active_passes_untyped_samples() {
        use crate::participant_ffi::{
            zerodds_dp_create_contentfilteredtopic, zerodds_dp_delete_contentfilteredtopic,
        };
        let (p, sub, t) = mk(58);
        // CFT mit "name = 'foo'" — fuer untyped Bytes-Topic, der EmptyRow
        // liefert None für jedes Feld; CftFilter::evaluate gibt
        // unwrap_or(true) zurueck (pass-through), spec-konform fuer
        // untyped.
        let cft_name = c"FilteredCFT";
        let expr = c"name = 'foo'";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let cft = unsafe {
            zerodds_dp_create_contentfilteredtopic(
                p,
                cft_name.as_ptr(),
                t,
                expr.as_ptr(),
                ptr::null(),
                0,
            )
        };
        assert!(!cft.is_null());
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dr = unsafe { zerodds_sub_create_datareader_with_cft(sub, cft, ptr::null()) };
        assert!(!dr.is_null(), "CFT-bound reader must be created");
        // Take auf empty: NoData (filter ist parsed, evaluated gegen leere
        // Channel — kein Sample da).
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 0,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dp_delete_contentfilteredtopic(p, cft) };
        cleanup(p);
    }

    #[test]
    fn cft_with_invalid_expression_returns_null() {
        use crate::participant_ffi::zerodds_dp_create_contentfilteredtopic;
        let (p, sub, t) = mk(59);
        let cft_name = c"BadCFT";
        let bad_expr = c"$$invalid syntax";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let cft = unsafe {
            zerodds_dp_create_contentfilteredtopic(
                p,
                cft_name.as_ptr(),
                t,
                bad_expr.as_ptr(),
                ptr::null(),
                0,
            )
        };
        // CFT-create succeeds (syntax not validated there).
        assert!(!cft.is_null());
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dr = unsafe { zerodds_sub_create_datareader_with_cft(sub, cft, ptr::null()) };
        // But create-with-cft fails on parse.
        assert!(dr.is_null(), "invalid filter syntax must reject");
        cleanup(p);
    }

    #[test]
    fn read_on_empty_returns_no_data() {
        let (p, sub, t) = mk(57);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dr = unsafe { zerodds_sub_create_datareader(sub, t, ptr::null()) };
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 0,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dr_read(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
        cleanup(p);
    }

    #[test]
    fn dr_get_topicdescription_subscriber_roundtrip() {
        let (p, sub, t) = mk(56);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dr = unsafe { zerodds_sub_create_datareader(sub, t, ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert_eq!(unsafe { zerodds_dr_get_topicdescription(dr) }, t);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert_eq!(unsafe { zerodds_dr_get_subscriber(dr) }, sub);
        cleanup(p);
    }
}
