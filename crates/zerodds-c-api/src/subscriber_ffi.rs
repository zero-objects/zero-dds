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

/// Returns the owning participant.
///
/// # Safety
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_get_participant(
    sub: *mut ZeroDdsSubscriber,
) -> *mut ZeroDdsDomainParticipant {
    if sub.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — sub NULL-checked above.
    unsafe { (*sub).participant }
}

/// `begin_access` (Spec §2.2.2.5.1.13). RC1: no-op marker for
/// coherent sets — the real set boundaries are on the reader wire path.
///
/// # Safety
/// `sub` valid.
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
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_end_access(sub: *mut ZeroDdsSubscriber) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Returns the list of active DataReaders.
///
/// # Safety
/// `sub`, `out`, `out_count` valid.
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
    // SAFETY: see fn # Safety doc — sub+out+out_count NULL-checked above; out[0..cap]
    // must be writeable (caller pledge).
    unsafe {
        let sb = &*sub;
        let drs = sb.datareaders.lock().map(|g| g.clone()).unwrap_or_default();
        let n = drs.len().min(cap);
        let dst = slice::from_raw_parts_mut(out, n);
        dst.copy_from_slice(&drs[..n]);
        *out_count = n;
    }
    ZeroDdsStatus::Ok as c_int
}

/// `notify_datareaders` (Spec §2.2.2.5.1.16). RC1: no-op — the listener bubble-up
/// runs per-reader, the subscriber aggregator is wired in the WP "Listeners-FFI".
///
/// # Safety
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_notify_datareaders(sub: *mut ZeroDdsSubscriber) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Creates a DataReader.
///
/// # Safety
/// `sub`, `topic` valid; `qos` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_datareader(
    sub: *mut ZeroDdsSubscriber,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataReaderQos,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || topic.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — sub+topic NULL-checked above; participant
    // from dp_create_subscriber; qos NULL-tolerant.
    unsafe {
        let sb = &*sub;
        let tt = &*topic;
        let dp_handle = sb.participant;
        if dp_handle.is_null() {
            return ptr::null_mut();
        }
        let dp = &*dp_handle;
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
            dr_qos_from_c(qos)
        };

        let cfg = UserReaderConfig {
            topic_name: tt.name.to_string(),
            type_name: tt.type_name.to_string(),
            reliable: matches!(qos.reliability.kind, ReliabilityKind::Reliable),
            durability: qos.durability.kind,
            deadline: qos.deadline.clone(),
            liveliness: qos.liveliness.clone(),
            ownership: qos.ownership.kind,
            // QB-cluster: plumb PARTITION from the Subscriber/reader QoS onto the
            // endpoint config so `partitions_overlap` (DDS 1.4 §2.2.3.10) gates
            // matching. Previously hardcoded empty.
            partition: qos.partition.names.clone(),
            user_data: qos.user_data.value.clone(),
            topic_data: qos.topic_data.value.clone(),
            group_data: qos.group_data.value.clone(),
            type_identifier: zerodds_types::TypeIdentifier::default(),
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            data_representation_offer: qos.data_representation.clone(),
        };
        let (eid, rx) = match rt.register_user_reader(cfg) {
            Ok(pair) => pair,
            Err(_) => return ptr::null_mut(),
        };
        let ownership = qos.ownership.kind;
        let dr = Box::new(ZeroDdsDataReader {
            subscriber: sub,
            topic,
            rt,
            eid,
            qos: Mutex::new(qos),
            rx: Mutex::new(rx),
            read_cache: Mutex::new(Vec::new()),
            cft_filter: None,
            ownership,
            instances: zerodds_dcps::instance_tracker::InstanceTracker::new(),
            partition_out: Mutex::new(Default::default()),
        });
        let dr_ptr = Box::into_raw(dr);
        if let Ok(mut list) = sb.datareaders.lock() {
            list.push(dr_ptr);
        }
        dr_ptr
    }
}

/// Creates a DataReader on a ContentFilteredTopic.
/// On every `take`/`read` the filter expression is evaluated
/// (Spec §2.2.2.3.3 + §2.2.2.5.2.5).
///
/// # Safety
/// `sub`, `cft` valid; `qos` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_datareader_with_cft(
    sub: *mut ZeroDdsSubscriber,
    cft: *mut crate::entities::ZeroDdsContentFilteredTopic,
    qos: *const ZeroDdsDataReaderQos,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || cft.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — sub+cft NULL-checked above; cft from
    // create_contentfilteredtopic; delegation to zerodds_sub_create_datareader;
    // dr is a fresh Box::into_raw that we collect again to set cft_filter.
    unsafe {
        let cft_ref = &*cft;
        let related_topic = cft_ref.related_topic;
        if related_topic.is_null() {
            return ptr::null_mut();
        }
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

        let schema: Vec<crate::entities::CftField> =
            cft_ref.schema.lock().map(|g| g.clone()).unwrap_or_default();
        let extensibility = cft_ref.extensibility.lock().map(|g| *g).unwrap_or_default();

        let dr = zerodds_sub_create_datareader(sub, related_topic, qos);
        if dr.is_null() {
            return ptr::null_mut();
        }
        let mut boxed = Box::from_raw(dr);
        boxed.cft_filter = Some(crate::entities::CftFilter {
            expr,
            params,
            schema,
            extensibility,
        });
        Box::into_raw(boxed)
    }
}

/// Deletes a DataReader.
///
/// # Safety
/// `sub`, `dr` valid and belonging together.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_delete_datareader(
    sub: *mut ZeroDdsSubscriber,
    dr: *mut ZeroDdsDataReader,
) -> c_int {
    if sub.is_null() || dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — sub+dr NULL-checked above; dr from sub_create_datareader.
    unsafe {
        if (*dr).subscriber != sub {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        let sb = &*sub;
        if let Ok(mut list) = sb.datareaders.lock() {
            let n = list.len();
            list.retain(|x| *x != dr);
            if list.len() == n {
                return ZeroDdsStatus::BadHandle as c_int;
            }
        }
        // Drop any zero-copy SHM map state keyed by this reader's (runtime, eid).
        #[cfg(feature = "flatdata-loan")]
        {
            let drr = &*dr;
            crate::shm_loan_ffi::forget_reader(&drr.rt, drr.eid);
        }
        let _ = Box::from_raw(dr);
    }
    ZeroDdsStatus::Ok as c_int
}

/// Look up a DataReader by topic name.
///
/// # Safety
/// `sub`, `topic_name` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_lookup_datareader(
    sub: *mut ZeroDdsSubscriber,
    topic_name: *const core::ffi::c_char,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — sub+topic_name NULL-checked above; datareaders
    // and their topic pointers from sub_create_datareader (Box::into_raw).
    unsafe {
        let cs = std::ffi::CStr::from_ptr(topic_name);
        let name = match cs.to_str() {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        };
        let sb = &*sub;
        if let Ok(list) = sb.datareaders.lock() {
            for &dr in list.iter() {
                if dr.is_null() {
                    continue;
                }
                let drr = &*dr;
                if !drr.topic.is_null() && (*drr.topic).name == name {
                    return dr;
                }
            }
        }
    }
    ptr::null_mut()
}

/// Deletes all DataReaders held by the subscriber.
///
/// # Safety
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_delete_contained_entities(
    sub: *mut ZeroDdsSubscriber,
) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — sub NULL-checked above; datareaders from
    // sub_create_datareader (Box::into_raw).
    unsafe {
        let drs: Vec<*mut ZeroDdsDataReader> = (*sub)
            .datareaders
            .lock()
            .map(|mut g| core::mem::take(&mut *g))
            .unwrap_or_default();
        for dr in drs {
            if !dr.is_null() {
                let _ = Box::from_raw(dr);
            }
        }
    }
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// DataReader: read / take + Sample-API
// ---------------------------------------------------------------------------

/// Returns the TopicDescription handle (in RC1: the topic pointer).
///
/// # Safety
/// `dr` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_topicdescription(
    dr: *mut ZeroDdsDataReader,
) -> *mut ZeroDdsTopic {
    if dr.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — dr NULL-checked above.
    unsafe { (*dr).topic }
}

/// Returns the subscriber.
///
/// # Safety
/// `dr` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_subscriber(
    dr: *mut ZeroDdsDataReader,
) -> *mut ZeroDdsSubscriber {
    if dr.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — dr NULL-checked above.
    unsafe { (*dr).subscriber }
}

/// SampleInfo (Spec §2.2.2.5.4 + DDS-PSM-Cxx §7.5.8.1).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsSampleInfo {
    /// Sample-state bit (1=READ, 2=NOT_READ).
    pub sample_state: u32,
    /// View state (1=NEW, 2=NOT_NEW).
    pub view_state: u32,
    /// Instance state (1=ALIVE, 2=NOT_ALIVE_DISPOSED, 4=NOT_ALIVE_NO_WRITERS).
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
    /// Instance handle.
    pub instance_handle: u64,
    /// Publication handle (writer-GUID hash).
    pub publication_handle: u64,
    /// `true` if the payload actually has data (vs. a lifecycle marker).
    pub valid_data: bool,
    /// XCDR version of the payload from the encapsulation header (RTPS 2.5
    /// §10.5): `0` = XCDR1 (CDR/PL_CDR), `1` = XCDR2 (CDR2/D_CDR2/PL_CDR2).
    /// The typed consumer needs this to apply the correct alignment rule.
    /// `0` for lifecycle markers (no payload).
    pub representation: u8,
    /// Wire byte order of the payload: `0` = little-endian (the canonical
    /// XCDR2 wire), `1` = big-endian — from the encapsulation representation
    /// identifier's low bit. The typed consumer dispatches its little-endian
    /// vs big-endian decoder on this. `0` for lifecycle markers.
    pub big_endian: u8,
}

/// Sample array (Spec §5 mini-spec).
#[repr(C)]
pub struct ZeroDdsSampleArray {
    /// Array of payload pointers.
    pub buffers: *mut *mut u8,
    /// Array of payload lengths.
    pub lengths: *mut usize,
    /// Array of SampleInfos.
    pub infos: *mut ZeroDdsSampleInfo,
    /// Number of samples.
    pub count: usize,
    /// Internal loan token (pointer to a `Vec<UserSample>` box). Freed
    /// by `return_loan`.
    pub loan_token: *mut core::ffi::c_void,
}

/// Internal loan memory: held until `return_loan` is called.
struct LoanMemory {
    payloads: Vec<Vec<u8>>,
    buffers: Vec<*mut u8>,
    lengths: Vec<usize>,
    infos: Vec<ZeroDdsSampleInfo>,
}

impl LoanMemory {
    fn new(samples: Vec<UserSample>) -> Box<Self> {
        // Single-sample helper path (no instance resolution): handle = NIL(0).
        let with_state: Vec<(UserSample, crate::entities::ReadSampleState, SampleHandle)> = samples
            .into_iter()
            .map(|s| (s, crate::entities::ReadSampleState::NotRead, 0u64))
            .collect();
        Self::from_state(with_state)
    }

    fn from_state(
        samples: Vec<(UserSample, crate::entities::ReadSampleState, SampleHandle)>,
    ) -> Box<Self> {
        let mut payloads: Vec<Vec<u8>> = Vec::with_capacity(samples.len());
        let mut infos: Vec<ZeroDdsSampleInfo> = Vec::with_capacity(samples.len());

        for (s, state, handle) in samples {
            let sample_state_bit = match state {
                crate::entities::ReadSampleState::Read => 1u32, // READ
                crate::entities::ReadSampleState::NotRead => 2u32, // NOT_READ
            };
            match s {
                UserSample::Alive {
                    payload,
                    writer_guid,
                    representation,
                    big_endian,
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
                        instance_handle: handle,
                        publication_handle: pub_handle,
                        valid_data: true,
                        representation,
                        big_endian: u8::from(big_endian),
                    });
                    payloads.push(payload.to_vec());
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
                        instance_handle: handle,
                        publication_handle: 0,
                        valid_data: false,
                        representation: 0,
                        big_endian: 0,
                    });
                    payloads.push(Vec::new());
                }
            }
        }

        // Buffers + lengths in separate Vecs so that their pointers are stable
        // (Vec<Vec<u8>>::as_mut_ptr() of the inner Vecs is stable
        // as long as the outer Vec does not re-allocate).
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

pub(crate) fn u64_from_guid(g: [u8; 16]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in g.iter() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Untyped per-instance KeyHash for the C-FFI take path. The C-FFI carries
/// no IDL key info, so the instance identity is derived from the full sample
/// payload: each distinct payload is one instance (Spec §2.2.2.5.4 — the
/// reader distinguishes instances by their KeyHash). For genuinely keyed
/// applications a writer that disposes an instance supplies the matching
/// `key_hash` directly, which the lifecycle path uses verbatim.
fn payload_key_hash(payload: &[u8]) -> [u8; 16] {
    let mut kh = [0u8; 16];
    // Two independent FNV-1a streams (different seeds) fill the 16-byte hash,
    // giving a low-collision instance identity for the untyped surface.
    let seeds: [u64; 2] = [0xcbf2_9ce4_8422_2325, 0x1000_0000_01b3_27d4];
    for (i, &seed) in seeds.iter().enumerate() {
        let mut h = seed;
        for &b in payload {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        kh[i * 8..i * 8 + 8].copy_from_slice(&h.to_le_bytes());
    }
    kh
}

/// Per-collected-sample side data computed in the take/read path: the
/// resolved `InstanceHandle` (Spec §2.2.2.5.4) surfaced in `SampleInfo`.
type SampleHandle = u64;

/// Walks the collected `Alive`/`Lifecycle` samples and (1) applies the
/// EXCLUSIVE-ownership arbitration via the validated dcps core
/// [`InstanceTracker::should_accept_sample_under_exclusive_ownership`]
/// (Spec §2.2.3.23) and (2) resolves the real per-instance `InstanceHandle`
/// for each surviving sample so dispose/unregister transitions are
/// distinguishable from C-FFI bindings.
fn resolve_instances_and_ownership(
    drr: &ZeroDdsDataReader,
    collected: Vec<(UserSample, crate::entities::ReadSampleState)>,
) -> Vec<(UserSample, crate::entities::ReadSampleState, SampleHandle)> {
    let exclusive = matches!(drr.ownership, zerodds_dcps::qos::OwnershipKind::Exclusive);
    let mut out = Vec::with_capacity(collected.len());
    for (s, state) in collected {
        match &s {
            UserSample::Alive {
                payload,
                writer_guid,
                writer_strength,
                ..
            } => {
                let kh = payload_key_hash(payload.as_ref());
                // Register the instance so the owner tracker has a slot
                // (mirrors dcps::Subscriber::passes_exclusive_ownership).
                let (handle, _new) = drr.instances.observe_sample(kh, payload.to_vec(), None);
                if exclusive
                    && !drr
                        .instances
                        .should_accept_sample_under_exclusive_ownership(
                            &kh,
                            *writer_guid,
                            *writer_strength,
                        )
                {
                    // Weaker writer's sample on an owned instance → drop.
                    continue;
                }
                out.push((s, state, handle.as_raw()));
            }
            UserSample::Lifecycle { key_hash, .. } => {
                // Lifecycle markers carry the real KeyHash; surface the same
                // handle as the matching Alive instance.
                let (handle, _new) =
                    drr.instances
                        .observe_sample(*key_hash, key_hash.to_vec(), None);
                out.push((s, state, handle.as_raw()));
            }
        }
    }
    out
}

/// Take: consumes samples from the cache + channel (Spec §2.2.2.5.3).
/// Goes through read_cache (sample-state READ + NOT_READ) and the channel
/// (sample-state NOT_READ), removes all delivered samples.
///
/// # Safety
/// `dr`, `out` valid.
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
    let limit = if max_samples == 0 {
        usize::MAX
    } else {
        max_samples
    };

    let mut collected: Vec<(UserSample, crate::entities::ReadSampleState)> = Vec::new();
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above; cache+rx live for
    // the reader; the out fields are written in one block.
    let empty_out = unsafe {
        let drr = &*dr;
        // 1) Empty the read cache (samples that `read()` has already seen).
        if let Ok(mut cache) = drr.read_cache.lock() {
            while collected.len() < limit && !cache.is_empty() {
                collected.push(cache.remove(0));
            }
        }
        // 2) Fresh samples from the channel.
        if let Ok(rx) = drr.rx.lock() {
            while collected.len() < limit {
                match rx.try_recv() {
                    Ok(s) => collected.push((s, crate::entities::ReadSampleState::NotRead)),
                    Err(_) => break,
                }
            }
        }
        // 3) ContentFilteredTopic filter (Spec §2.2.2.3.3).
        if let Some(filter) = &drr.cft_filter {
            collected.retain(|(s, _)| match s {
                UserSample::Alive { payload, .. } => filter.evaluate(payload.as_ref()),
                UserSample::Lifecycle { .. } => true,
            });
        }
        if collected.is_empty() {
            (*out).buffers = ptr::null_mut();
            (*out).lengths = ptr::null_mut();
            (*out).infos = ptr::null_mut();
            (*out).count = 0;
            (*out).loan_token = ptr::null_mut();
            true
        } else {
            false
        }
    };
    if empty_out {
        return ZeroDdsStatus::NoData as c_int;
    }
    // 4) EXCLUSIVE-ownership arbitration (Spec §2.2.3.23) + per-instance
    //    InstanceHandle resolution (Spec §2.2.2.5.4).
    // SAFETY: dr NULL-checked at entry; the reader lives for this call.
    let resolved = unsafe { resolve_instances_and_ownership(&*dr, collected) };
    if resolved.is_empty() {
        // All samples filtered out by exclusive-ownership arbitration.
        // SAFETY: out NULL-checked at entry.
        unsafe {
            (*out).buffers = ptr::null_mut();
            (*out).lengths = ptr::null_mut();
            (*out).infos = ptr::null_mut();
            (*out).count = 0;
            (*out).loan_token = ptr::null_mut();
        }
        return ZeroDdsStatus::NoData as c_int;
    }
    finalize_loan(out, resolved)
}

/// Read: non-destructive variant of take (Spec §2.2.2.5.3).
/// Returns samples from the cache + channel, but ALL of them remain afterwards
/// in the read cache with sample-state = READ.
///
/// # Safety
/// `dr`, `out` valid.
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
    let limit = if max_samples == 0 {
        usize::MAX
    } else {
        max_samples
    };

    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    let (collected, empty_out) = unsafe {
        let drr = &*dr;
        // Pull fresh samples from the channel into the cache (with the NOT_READ marker).
        if let (Ok(rx), Ok(mut cache)) = (drr.rx.lock(), drr.read_cache.lock()) {
            while let Ok(s) = rx.try_recv() {
                let pass = if let Some(filter) = &drr.cft_filter {
                    match &s {
                        UserSample::Alive { payload, .. } => filter.evaluate(payload.as_ref()),
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
        // Read the first `limit` from the cache (clone), mark as READ.
        let collected: Vec<(UserSample, crate::entities::ReadSampleState)> =
            if let Ok(mut cache) = drr.read_cache.lock() {
                let n = cache.len().min(limit);
                let out_collected: Vec<_> = cache
                    .iter()
                    .take(n)
                    .map(|(s, st)| (s.clone(), *st))
                    .collect();
                for entry in cache.iter_mut().take(n) {
                    entry.1 = crate::entities::ReadSampleState::Read;
                }
                out_collected
            } else {
                Vec::new()
            };
        let empty = collected.is_empty();
        if empty {
            (*out).buffers = ptr::null_mut();
            (*out).lengths = ptr::null_mut();
            (*out).infos = ptr::null_mut();
            (*out).count = 0;
            (*out).loan_token = ptr::null_mut();
        }
        (collected, empty)
    };
    if empty_out {
        return ZeroDdsStatus::NoData as c_int;
    }
    // EXCLUSIVE-ownership arbitration (Spec §2.2.3.23) + per-instance
    // InstanceHandle resolution (Spec §2.2.2.5.4), as in `take`.
    // SAFETY: dr NULL-checked at entry; the reader lives for this call.
    let resolved = unsafe { resolve_instances_and_ownership(&*dr, collected) };
    if resolved.is_empty() {
        // SAFETY: out NULL-checked at entry.
        unsafe {
            (*out).buffers = ptr::null_mut();
            (*out).lengths = ptr::null_mut();
            (*out).infos = ptr::null_mut();
            (*out).count = 0;
            (*out).loan_token = ptr::null_mut();
        }
        return ZeroDdsStatus::NoData as c_int;
    }
    finalize_loan(out, resolved)
}

fn finalize_loan(
    out: *mut ZeroDdsSampleArray,
    collected: Vec<(UserSample, crate::entities::ReadSampleState, SampleHandle)>,
) -> c_int {
    let mut loan = LoanMemory::from_state(collected);
    let buffers_ptr = loan.buffers.as_mut_ptr();
    let lengths_ptr = loan.lengths.as_mut_ptr();
    let infos_ptr = loan.infos.as_mut_ptr();
    let count = loan.payloads.len();
    let token = Box::into_raw(loan) as *mut core::ffi::c_void;
    // SAFETY: out was NULL-checked by dr_take/dr_read before the call; the caller pledge
    // guarantees a valid ZeroDdsSampleArray struct.
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
/// `dr`, `out_buf`, `out_len`, `out_info` valid.
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
    // SAFETY: see fn # Safety doc — dr+out_buf+out_len+out_info NULL-checked above.
    let s = unsafe {
        let drr = &*dr;
        match drr.rx.lock().ok().and_then(|rx| rx.try_recv().ok()) {
            Some(s) => s,
            None => return ZeroDdsStatus::NoData as c_int,
        }
    };
    let loan = LoanMemory::new(alloc::vec![s]);
    // SAFETY: out_*-Pointer NULL-checked oben.
    unsafe {
        *out_buf = loan.buffers[0];
        *out_len = loan.lengths[0];
        *out_info = loan.infos[0];
    }
    // Single-sample take: the loan hangs off out_buf. The caller must
    // use the `take` loan path if it wants to return memory —
    // here we transfer ownership to the caller via Box::into_raw +
    // a memory-leak path: documented in the mini-spec, the caller MUST
    // call `zerodds_dr_return_loan` with a SampleArray { loan_token: <token> }.
    // We anchor the token in a thread-local variable —
    // or better: we do not return it via out_info. This
    // RC1 variant is therefore leak-tolerant: the loan becomes droppable.
    let _ = Box::into_raw(loan);
    ZeroDdsStatus::Ok as c_int
}

/// Read next single sample. Identical to take_next_sample in RC1.
///
/// # Safety
/// Like `take_next_sample`.
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

/// Filters a `SampleArray` in place to a single
/// `instance_handle`. Samples that do not match are removed from the
/// arrays; `count` is adjusted.
///
/// # Safety
/// `arr` valid; `arr.buffers/lengths/infos[0..count]` lesbar.
pub fn sample_array_filter_instance(arr: *mut ZeroDdsSampleArray, handle: u64) -> c_int {
    if arr.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: arr NULL-checked above; arr comes from dr_take/dr_read; all sub-pointers
    // (buffers/lengths/infos) were written by finalize_loan and are valid 0..count.
    unsafe {
        let count = (*arr).count;
        if count == 0 {
            return ZeroDdsStatus::Ok as c_int;
        }
        let buffers = (*arr).buffers;
        let lengths = (*arr).lengths;
        let infos = (*arr).infos;
        if buffers.is_null() || lengths.is_null() || infos.is_null() {
            return ZeroDdsStatus::Ok as c_int;
        }
        let mut write_idx: usize = 0;
        for read_idx in 0..count {
            let info = *infos.add(read_idx);
            if info.instance_handle == handle {
                if write_idx != read_idx {
                    *buffers.add(write_idx) = *buffers.add(read_idx);
                    *lengths.add(write_idx) = *lengths.add(read_idx);
                    *infos.add(write_idx) = info;
                }
                write_idx += 1;
            }
        }
        (*arr).count = write_idx;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Filters to the "next" instance > prev_handle. Returns only
/// samples whose instance_handle is > prev_handle AND minimal.
pub fn sample_array_filter_next_instance(arr: *mut ZeroDdsSampleArray, prev_handle: u64) -> c_int {
    if arr.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: arr from dr_take/dr_read; infos valid 0..count.
    let next_handle = unsafe {
        let count = (*arr).count;
        if count == 0 {
            return ZeroDdsStatus::Ok as c_int;
        }
        let infos = (*arr).infos;
        if infos.is_null() {
            return ZeroDdsStatus::Ok as c_int;
        }
        let mut next: Option<u64> = None;
        for read_idx in 0..count {
            let h = (*infos.add(read_idx)).instance_handle;
            if h > prev_handle && next.is_none_or(|n| h < n) {
                next = Some(h);
            }
        }
        next
    };
    match next_handle {
        Some(h) => sample_array_filter_instance(arr, h),
        None => {
            // SAFETY: arr NULL-checked oben.
            unsafe { (*arr).count = 0 };
            ZeroDdsStatus::Ok as c_int
        }
    }
}

/// Filters to a sample_state/view_state/instance_state bitmask.
/// Mask=0 means "all allowed".
pub fn sample_array_filter_states(
    arr: *mut ZeroDdsSampleArray,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
) -> c_int {
    if arr.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: arr from dr_take/dr_read; all sub-pointers valid 0..count.
    unsafe {
        let count = (*arr).count;
        if count == 0 {
            return ZeroDdsStatus::Ok as c_int;
        }
        let buffers = (*arr).buffers;
        let lengths = (*arr).lengths;
        let infos = (*arr).infos;
        if buffers.is_null() || lengths.is_null() || infos.is_null() {
            return ZeroDdsStatus::Ok as c_int;
        }
        let mut write_idx = 0usize;
        for read_idx in 0..count {
            let info = *infos.add(read_idx);
            let s_ok = sample_states == 0 || (sample_states & info.sample_state) != 0;
            let v_ok = view_states == 0 || (view_states & info.view_state) != 0;
            let i_ok = instance_states == 0 || (instance_states & info.instance_state) != 0;
            if s_ok && v_ok && i_ok {
                if write_idx != read_idx {
                    *buffers.add(write_idx) = *buffers.add(read_idx);
                    *lengths.add(write_idx) = *lengths.add(read_idx);
                    *infos.add(write_idx) = info;
                }
                write_idx += 1;
            }
        }
        (*arr).count = write_idx;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Returns loaned sample buffers.
///
/// # Safety
/// `arr` must come from a previous `zerodds_dr_take`/`read`,
/// `loan_token` must still be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_return_loan(
    _dr: *mut ZeroDdsDataReader,
    arr: *mut ZeroDdsSampleArray,
) -> c_int {
    if arr.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — arr NULL-checked above; loan_token from
    // LoanMemory::new Box::into_raw in dr_take/read.
    unsafe {
        let token = (*arr).loan_token;
        if !token.is_null() {
            let _ = Box::from_raw(token as *mut LoanMemory);
        }
        (*arr).buffers = ptr::null_mut();
        (*arr).lengths = ptr::null_mut();
        (*arr).infos = ptr::null_mut();
        (*arr).count = 0;
        (*arr).loan_token = ptr::null_mut();
    }
    ZeroDdsStatus::Ok as c_int
}

/// Waits until `min` matched publications or a timeout.
///
/// # Safety
/// `dr` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_wait_for_matched(
    dr: *mut ZeroDdsDataReader,
    min: i32,
    timeout_ms: u64,
) -> c_int {
    if dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — dr NULL-checked above.
    let (rt, eid) = unsafe { ((*dr).rt.clone(), (*dr).eid) };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let n = rt.user_reader_matched_count(eid) as i32;
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
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_liveliness_changed_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsLivelinessChangedStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let (alive, alive_count, not_alive_count) = drr.rt.user_reader_liveliness_status(drr.eid);
        let _ = alive;
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
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_subscription_matched_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSubscriptionMatchedStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let n = drr.rt.user_reader_matched_count(drr.eid) as i32;
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
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_requested_deadline_missed_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsRequestedDeadlineMissedStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let n = drr.rt.user_reader_requested_deadline_missed(drr.eid);
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
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_requested_incompatible_qos_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsRequestedIncompatibleQosStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let st = drr.rt.user_reader_requested_incompatible_qos(drr.eid);
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
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_sample_lost_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleLostStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let n = drr.rt.user_reader_sample_lost(drr.eid);
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
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_sample_rejected_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleRejectedStatus,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    use zerodds_dcps::status::SampleRejectedStatusKind;
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let st = drr.rt.user_reader_sample_rejected(drr.eid);
        let reason: u32 = match st.last_reason {
            SampleRejectedStatusKind::NotRejected => 0,
            SampleRejectedStatusKind::RejectedByInstancesLimit => 1,
            SampleRejectedStatusKind::RejectedBySamplesLimit => 2,
            SampleRejectedStatusKind::RejectedBySamplesPerInstanceLimit => 3,
        };
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
        let n = c"SubT";
        let tn = c"SubTy";
        // SAFETY: f from dpf_get_instance, n+tn statically valid.
        unsafe {
            let p = zerodds_dpf_create_participant(f, domain, ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            (p, sub, t)
        }
    }
    fn cleanup(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: p from mk; f static.
        unsafe {
            zerodds_dp_delete_contained_entities(p);
            zerodds_dpf_delete_participant(f, p);
        }
    }

    #[test]
    fn create_delete_datareader() {
        let (p, sub, t) = mk(51);
        // SAFETY: sub+t from mk, valid until cleanup.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert!(!dr.is_null());
            let rc = zerodds_sub_delete_datareader(sub, dr);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        }
        cleanup(p);
    }

    #[test]
    fn lookup_datareader_finds_existing() {
        let (p, sub, t) = mk(52);
        let n = c"SubT";
        // SAFETY: sub+t from mk; n static.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let f = zerodds_sub_lookup_datareader(sub, n.as_ptr());
            assert_eq!(f, dr);
        }
        cleanup(p);
    }

    #[test]
    fn from_state_fills_representation_and_byte_order() {
        // A loan-batch sample carries its wire representation + byte order out
        // to the FFI consumer (TS/C++) so it can dispatch decode vs decode_be.
        use zerodds_dcps::sample_bytes::SampleBytes;
        let alive = UserSample::Alive {
            payload: SampleBytes::from_vec(vec![1, 2, 3, 4]),
            writer_guid: [0xAB; 16],
            writer_strength: 0,
            representation: 1, // XCDR2
            big_endian: true,  // big-endian peer
            source_timestamp: None,
            source_sequence_number: -1,
        };
        let lifecycle = UserSample::Lifecycle {
            key_hash: [0; 16],
            kind: zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed,
        };
        let mem = LoanMemory::from_state(vec![
            (alive, crate::entities::ReadSampleState::NotRead, 7u64),
            (lifecycle, crate::entities::ReadSampleState::NotRead, 8u64),
        ]);
        assert_eq!(mem.infos.len(), 2);
        // Alive sample reflects the wire encap.
        assert!(mem.infos[0].valid_data);
        assert_eq!(mem.infos[0].representation, 1);
        assert_eq!(mem.infos[0].big_endian, 1);
        // Lifecycle marker has no payload → neutral 0/0.
        assert!(!mem.infos[1].valid_data);
        assert_eq!(mem.infos[1].representation, 0);
        assert_eq!(mem.infos[1].big_endian, 0);
    }

    #[test]
    fn take_on_empty_returns_no_data() {
        let (p, sub, t) = mk(53);
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 0,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: sub+t from mk; arr lives for the test.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let rc = zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
        }
        assert_eq!(arr.count, 0);
        cleanup(p);
    }

    #[test]
    fn return_loan_clears_array() {
        let (p, sub, t) = mk(54);
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 7,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: sub+t from mk; arr lives for the test.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let rc = zerodds_dr_return_loan(dr, &mut arr);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        }
        assert_eq!(arr.count, 0);
        cleanup(p);
    }

    #[test]
    fn statuses_default_ok() {
        let (p, sub, t) = mk(55);
        let mut a = ZeroDdsLivelinessChangedStatus::default();
        let mut b = ZeroDdsSubscriptionMatchedStatus::default();
        let mut c = ZeroDdsRequestedDeadlineMissedStatus::default();
        let mut d = ZeroDdsRequestedIncompatibleQosStatus::default();
        let mut e = ZeroDdsSampleLostStatus::default();
        let mut g = ZeroDdsSampleRejectedStatus::default();
        // SAFETY: sub+t from mk; status slots on the stack.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let ok = ZeroDdsStatus::Ok as c_int;
            assert_eq!(zerodds_dr_get_liveliness_changed_status(dr, &mut a), ok);
            assert_eq!(zerodds_dr_get_subscription_matched_status(dr, &mut b), ok);
            assert_eq!(
                zerodds_dr_get_requested_deadline_missed_status(dr, &mut c),
                ok
            );
            assert_eq!(
                zerodds_dr_get_requested_incompatible_qos_status(dr, &mut d),
                ok
            );
            assert_eq!(zerodds_dr_get_sample_lost_status(dr, &mut e), ok);
            assert_eq!(zerodds_dr_get_sample_rejected_status(dr, &mut g), ok);
        }
        cleanup(p);
    }

    #[test]
    fn cft_filter_active_passes_untyped_samples() {
        use crate::participant_ffi::{
            zerodds_dp_create_contentfilteredtopic, zerodds_dp_delete_contentfilteredtopic,
        };
        let (p, sub, t) = mk(58);
        let cft_name = c"FilteredCFT";
        let expr = c"name = 'foo'";
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 0,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: p+sub+t from mk; cft_name+expr static; arr lives for the test.
        // CFT with "name = 'foo'" — for an untyped bytes topic EmptyRow returns None;
        // CftFilter::evaluate returns unwrap_or(true) (pass-through).
        unsafe {
            let cft = zerodds_dp_create_contentfilteredtopic(
                p,
                cft_name.as_ptr(),
                t,
                expr.as_ptr(),
                ptr::null(),
                0,
            );
            assert!(!cft.is_null());
            let dr = zerodds_sub_create_datareader_with_cft(sub, cft, ptr::null());
            assert!(!dr.is_null(), "CFT-bound reader must be created");
            let rc = zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
            zerodds_dp_delete_contentfilteredtopic(p, cft);
        }
        cleanup(p);
    }

    #[test]
    fn cft_with_invalid_expression_returns_null() {
        use crate::participant_ffi::zerodds_dp_create_contentfilteredtopic;
        let (p, sub, t) = mk(59);
        let cft_name = c"BadCFT";
        let bad_expr = c"$$invalid syntax";
        // SAFETY: p+sub+t from mk; cft_name+bad_expr static.
        unsafe {
            let cft = zerodds_dp_create_contentfilteredtopic(
                p,
                cft_name.as_ptr(),
                t,
                bad_expr.as_ptr(),
                ptr::null(),
                0,
            );
            // CFT-create succeeds (syntax not validated there).
            assert!(!cft.is_null());
            let dr = zerodds_sub_create_datareader_with_cft(sub, cft, ptr::null());
            // But create-with-cft fails on parse.
            assert!(dr.is_null(), "invalid filter syntax must reject");
        }
        cleanup(p);
    }

    #[test]
    fn read_on_empty_returns_no_data() {
        let (p, sub, t) = mk(57);
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 0,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: sub+t from mk; arr lives for the test.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let rc = zerodds_dr_read(dr, &mut arr, 10, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
        }
        cleanup(p);
    }

    #[test]
    fn dr_get_topicdescription_subscriber_roundtrip() {
        let (p, sub, t) = mk(56);
        // SAFETY: sub+t from mk.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert_eq!(zerodds_dr_get_topicdescription(dr), t);
            assert_eq!(zerodds_dr_get_subscriber(dr), sub);
        }
        cleanup(p);
    }

    // ===================================================================
    // QR-cluster regression tests (#77): exclusive-ownership arbitration,
    // CFT payload decode, keyed-lifecycle InstanceHandle on the take path.
    // ===================================================================

    use crate::entities::ZeroDdsDataReader;
    use zerodds_dcps::sample_bytes::SampleBytes;
    use zerodds_rtps::history_cache::ChangeKind;

    /// Builds a standalone DataReader with a controlled sample channel so a
    /// test can inject `UserSample`s with explicit writer GUID/strength —
    /// i.e. a real same-runtime writer→reader observation of the QoS effect
    /// on the C-FFI take path. `rt`/`eid` come from the participant; the
    /// reader is NOT registered in the subscriber list, so the test drops it
    /// explicitly.
    fn mk_channel_reader(
        p: *mut ZeroDdsDomainParticipant,
        sub: *mut ZeroDdsSubscriber,
        topic: *mut ZeroDdsTopic,
        ownership: zerodds_dcps::qos::OwnershipKind,
        cft_filter: Option<crate::entities::CftFilter>,
    ) -> (*mut ZeroDdsDataReader, mpsc::Sender<UserSample>) {
        // SAFETY: p from mk; dp.rt is Some for an online participant.
        let rt = unsafe { (*p).rt.as_ref().expect("participant runtime").clone() };
        let eid = zerodds_rtps::wire_types::EntityId::user_reader_with_key([9, 9, 9]);
        let (tx, rx) = mpsc::channel::<UserSample>();
        let dr = Box::new(ZeroDdsDataReader {
            subscriber: sub,
            topic,
            rt,
            eid,
            qos: Mutex::new(DataReaderQos::default()),
            rx: Mutex::new(rx),
            read_cache: Mutex::new(Vec::new()),
            cft_filter,
            ownership,
            instances: zerodds_dcps::instance_tracker::InstanceTracker::new(),
            partition_out: Mutex::new(Default::default()),
        });
        (Box::into_raw(dr), tx)
    }

    fn alive(payload: Vec<u8>, guid_byte: u8, strength: i32) -> UserSample {
        let mut g = [0u8; 16];
        g[0] = guid_byte;
        UserSample::Alive {
            payload: SampleBytes::from_vec(payload),
            writer_guid: g,
            writer_strength: strength,
            representation: 1,
            big_endian: false,
            source_timestamp: None,
            source_sequence_number: -1,
        }
    }

    fn empty_arr() -> ZeroDdsSampleArray {
        ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 0,
            loan_token: ptr::null_mut(),
        }
    }

    /// EXCLUSIVE ownership (Spec §2.2.3.23): two writers send to the SAME
    /// instance (identical payload-key). The stronger writer (strength 20)
    /// owns the instance; the weaker writer's (strength 5) sample on that
    /// instance is filtered out on the take path. Routes through the
    /// validated dcps `InstanceTracker::should_accept_sample_under_exclusive_ownership`.
    #[test]
    fn exclusive_ownership_drops_weaker_writer_on_take() {
        let (p, sub, t) = mk(60);
        let (dr, tx) =
            mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Exclusive, None);
        // Same instance payload from a strong writer first.
        let inst = alloc::vec![0x10u8, 0x20, 0x30, 0x40];
        tx.send(alive(inst.clone(), 0x01, 20)).unwrap();
        // Weaker writer, same instance → must be rejected.
        tx.send(alive(inst.clone(), 0x02, 5)).unwrap();
        // A second sample from the strong owner → accepted.
        tx.send(alive(inst.clone(), 0x01, 20)).unwrap();

        let mut arr = empty_arr();
        // SAFETY: dr+arr valid for the test.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        // Only the two strong-writer samples survive; weaker one filtered.
        assert_eq!(arr.count, 2, "weaker writer's sample must be dropped");
        // SAFETY: arr from take; dr valid.
        unsafe {
            let pubh = (*arr.infos.add(0)).publication_handle;
            assert_eq!(
                (*arr.infos.add(1)).publication_handle,
                pubh,
                "both surviving samples come from the same (strong) writer"
            );
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// SHARED ownership (default): both writers' samples are delivered (no
    /// arbitration). Confirms the filter is gated on EXCLUSIVE only.
    #[test]
    fn shared_ownership_delivers_all_writers() {
        let (p, sub, t) = mk(61);
        let (dr, tx) = mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Shared, None);
        let inst = alloc::vec![1u8, 2, 3, 4];
        tx.send(alive(inst.clone(), 0x01, 20)).unwrap();
        tx.send(alive(inst.clone(), 0x02, 5)).unwrap();
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 2, "shared ownership delivers every writer");
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// CFT (Spec §2.2.2.3.3): a `seq > 4` filter with an Int32 schema on the
    /// leading member genuinely filters the decoded payload. Samples with
    /// seq <= 4 are dropped; seq > 4 pass.
    #[test]
    fn cft_seq_filter_decodes_and_filters_payload() {
        use crate::entities::{CftField, CftFieldKind, CftFilter};
        let (p, sub, t) = mk(62);
        let expr = zerodds_sql_filter::parse("seq > 4").expect("parse seq>4");
        let filter = CftFilter {
            expr,
            params: Vec::new(),
            schema: alloc::vec![CftField {
                name: "seq".into(),
                kind: CftFieldKind::Int32,
            }],
            extensibility: crate::entities::CftExtensibility::Final,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );
        // XCDR2 little-endian i32 payloads: 3 (drop), 7 (keep), 5 (keep).
        for v in [3i32, 7, 5] {
            tx.send(alive(v.to_le_bytes().to_vec(), 0x01, 0)).unwrap();
        }
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 2, "only seq=7 and seq=5 pass seq>4");
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// CFT string field: `name = 'foo'` over a length-prefixed CDR string
    /// payload genuinely filters.
    #[test]
    fn cft_string_filter_decodes_payload() {
        use crate::entities::{CftField, CftFieldKind, CftFilter};
        let (p, sub, t) = mk(63);
        let expr = zerodds_sql_filter::parse("name = 'foo'").expect("parse name=foo");
        let filter = CftFilter {
            expr,
            params: Vec::new(),
            schema: alloc::vec![CftField {
                name: "name".into(),
                kind: CftFieldKind::StringField,
            }],
            extensibility: crate::entities::CftExtensibility::Final,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );
        // CDR string: u32 len (incl NUL) + bytes + NUL, XCDR LE.
        let cdr_string = |s: &str| -> Vec<u8> {
            let mut v = Vec::new();
            let len = (s.len() + 1) as u32;
            v.extend_from_slice(&len.to_le_bytes());
            v.extend_from_slice(s.as_bytes());
            v.push(0);
            v
        };
        tx.send(alive(cdr_string("foo"), 0x01, 0)).unwrap();
        tx.send(alive(cdr_string("bar"), 0x01, 0)).unwrap();
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 1, "only name='foo' passes");
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// QB-rest regression (#79, issue 3): CFT untyped positional schema for an
    /// `@appendable` type. In XCDR2 an @appendable/@mutable aggregate is
    /// prefixed with a 4-byte DHEADER (XTypes 1.3 §7.4.3.4.2) before the first
    /// member, so the positional decoder must skip those 4 bytes — otherwise it
    /// reads the DHEADER as the first column and every comparison resolves at
    /// the wrong offset. With `CftExtensibility::Appendable` set, both a numeric
    /// (`seq` int32) and a string (`name`) column resolve, so the filter
    /// `seq > 4 AND name = 'keep'` genuinely filters.
    #[test]
    fn cft_appendable_dheader_offset_resolves_columns() {
        use crate::entities::{CftExtensibility, CftField, CftFieldKind, CftFilter};
        let (p, sub, t) = mk(65);
        let expr = zerodds_sql_filter::parse("seq > 4 AND name = 'keep'")
            .expect("parse appendable filter");
        let filter = CftFilter {
            expr,
            params: Vec::new(),
            schema: alloc::vec![
                CftField {
                    name: "seq".into(),
                    kind: CftFieldKind::Int32,
                },
                CftField {
                    name: "name".into(),
                    kind: CftFieldKind::StringField,
                },
            ],
            // @appendable → leading 4-byte DHEADER is skipped.
            extensibility: CftExtensibility::Appendable,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );

        // Build an @appendable XCDR2 body: 4-byte DHEADER (object length) then
        // the members. After the i32 `seq`, the `name` string is 4-byte aligned
        // (already aligned here: 4 DHEADER + 4 seq = offset 8). CDR string =
        // u32 length (incl NUL) + bytes + NUL.
        let appendable = |seq: i32, name: &str| -> Vec<u8> {
            let mut body = Vec::new();
            body.extend_from_slice(&seq.to_le_bytes());
            let len = (name.len() + 1) as u32;
            body.extend_from_slice(&len.to_le_bytes());
            body.extend_from_slice(name.as_bytes());
            body.push(0);
            // DHEADER = byte length of the member block that follows.
            let mut v = Vec::new();
            v.extend_from_slice(&(body.len() as u32).to_le_bytes());
            v.extend_from_slice(&body);
            v
        };

        // seq=3,"keep" → drop (seq<=4); seq=7,"drop" → drop (name); seq=9,"keep" → keep.
        tx.send(alive(appendable(3, "keep"), 0x01, 1)).unwrap();
        tx.send(alive(appendable(7, "drop"), 0x01, 1)).unwrap();
        tx.send(alive(appendable(9, "keep"), 0x01, 1)).unwrap();

        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(
            arr.count, 1,
            "only seq=9,name='keep' passes once the DHEADER offset is honored"
        );
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// Control for the DHEADER fix: the SAME @appendable payload with the
    /// extensibility left at `Final` (offset 0) reads the DHEADER as `seq` and
    /// thus does NOT resolve the columns correctly — proving the offset is
    /// load-bearing. The DHEADER value (a small object length) is < 5, so a
    /// `seq > 4` filter on the misread offset rejects the sample.
    #[test]
    fn cft_appendable_payload_misreads_without_dheader_skip() {
        use crate::entities::{CftExtensibility, CftField, CftFieldKind, CftFilter};
        let (p, sub, t) = mk(66);
        let expr = zerodds_sql_filter::parse("seq > 4").expect("parse seq>4");
        let filter = CftFilter {
            expr,
            params: Vec::new(),
            schema: alloc::vec![CftField {
                name: "seq".into(),
                kind: CftFieldKind::Int32,
            }],
            // WRONG on purpose: payload is @appendable but declared @final.
            extensibility: CftExtensibility::Final,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );
        // @appendable body: DHEADER(=4) + seq(=9). Read at offset 0 the decoder
        // sees the DHEADER (4) as `seq`, so `seq > 4` is FALSE and the sample is
        // dropped — even though the real seq is 9.
        let mut payload = Vec::new();
        payload.extend_from_slice(&4u32.to_le_bytes()); // DHEADER
        payload.extend_from_slice(&9i32.to_le_bytes()); // real seq
        tx.send(alive(payload, 0x01, 1)).unwrap();
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        // Misread offset → seq looks like 4 → seq>4 false → NoData.
        assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
        assert_eq!(arr.count, 0);
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// Keyed lifecycle (Spec §2.2.2.5.4): the take path surfaces a real,
    /// non-zero, per-instance InstanceHandle. Two distinct payloads yield two
    /// distinct handles; a DISPOSED lifecycle marker for an instance carries
    /// the SAME handle as that instance's alive sample AND reports
    /// instance_state = NOT_ALIVE_DISPOSED (2).
    #[test]
    fn keyed_lifecycle_surfaces_real_instance_handle() {
        let (p, sub, t) = mk(64);
        let (dr, tx) = mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Shared, None);
        let inst_a = alloc::vec![0xAAu8, 0xAA, 0xAA, 0xAA];
        let inst_b = alloc::vec![0xBBu8, 0xBB, 0xBB, 0xBB];
        tx.send(alive(inst_a.clone(), 0x01, 0)).unwrap();
        tx.send(alive(inst_b.clone(), 0x01, 0)).unwrap();

        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 2);
        // SAFETY: take returned count==2, so infos[0..2] are initialized.
        let (handle_a, handle_b) = unsafe {
            let h0 = (*arr.infos.add(0)).instance_handle;
            let h1 = (*arr.infos.add(1)).instance_handle;
            (h0, h1)
        };
        assert_ne!(handle_a, 0, "alive instance handle must be non-zero");
        assert_ne!(handle_b, 0, "alive instance handle must be non-zero");
        assert_ne!(
            handle_a, handle_b,
            "distinct instances must have distinct handles"
        );
        // SAFETY: return first loan before the next take.
        unsafe { zerodds_dr_return_loan(dr, &mut arr) };

        // Now dispose instance A via a Lifecycle marker carrying its KeyHash.
        // The C-FFI derives the alive instance handle from the payload; the
        // dispose path must produce the SAME handle so the transition is
        // correlatable. We reconstruct A's payload-key hash the same way the
        // take path does.
        let kh_a = payload_key_hash(&inst_a);
        tx.send(UserSample::Lifecycle {
            key_hash: kh_a,
            kind: ChangeKind::NotAliveDisposed,
        })
        .unwrap();
        let mut arr2 = empty_arr();
        // SAFETY: dr+arr2 valid.
        let rc2 = unsafe { zerodds_dr_take(dr, &mut arr2, 10, 0, 0, 0) };
        assert_eq!(rc2, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr2.count, 1);
        // SAFETY: arr2 from take.
        unsafe {
            let info = *arr2.infos.add(0);
            assert_eq!(
                info.instance_state, 2,
                "DISPOSED → NOT_ALIVE_DISPOSED state"
            );
            assert!(!info.valid_data, "lifecycle marker carries no data");
            assert_eq!(
                info.instance_handle, handle_a,
                "dispose marker handle must match the alive instance handle"
            );
            zerodds_dr_return_loan(dr, &mut arr2);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }
}
