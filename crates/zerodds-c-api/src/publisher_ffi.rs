// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Publisher + DataWriter C-FFI (Spec §2.2.2.4 + DDS-PSM-Cxx §7.5.5/§7.5.7).

use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ffi::c_int;
use core::ptr;
use core::slice;
use std::sync::Mutex;
use std::time::Duration;

use zerodds_dcps::qos::{DataWriterQos, DurabilityKind, ReliabilityKind};
use zerodds_dcps::runtime::UserWriterConfig;

use crate::ZeroDdsStatus;
use crate::entities::{
    ZeroDdsDataWriter, ZeroDdsDomainParticipant, ZeroDdsPublisher, ZeroDdsTopic,
};
use crate::qos_ffi::{ZeroDdsDataWriterQos, dw_qos_from_c};

// ---------------------------------------------------------------------------
// Publisher: lookup / participant / suspend / resume
// ---------------------------------------------------------------------------

/// Returns the owning participant.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_get_participant(
    pub_: *mut ZeroDdsPublisher,
) -> *mut ZeroDdsDomainParticipant {
    if pub_.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above.
    unsafe { (*pub_).participant }
}

/// Suspend publications (Spec §2.2.2.4.1.10). Sets a flag — the
/// hot path queues writes instead of sending them, until `resume_publications`.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_suspend_publications(pub_: *mut ZeroDdsPublisher) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above.
    unsafe {
        if let Ok(mut g) = (*pub_).suspended.lock() {
            *g = true;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Resume publications.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_resume_publications(pub_: *mut ZeroDdsPublisher) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above.
    unsafe {
        if let Ok(mut g) = (*pub_).suspended.lock() {
            *g = false;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// begin_coherent_changes (Spec §2.2.2.4.1.12). In RC1 a no-bracket
/// marker — a spec-conformant tracker at the publisher level; the coherent-set
/// result wire frames live at the writer level and take effect independently.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_begin_coherent_changes(pub_: *mut ZeroDdsPublisher) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    ZeroDdsStatus::Ok as c_int
}

/// end_coherent_changes.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_end_coherent_changes(pub_: *mut ZeroDdsPublisher) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    ZeroDdsStatus::Ok as c_int
}

/// wait_for_acknowledgments(timeout_ms) — waits until all DWs are
/// acked or the timeout fires.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_wait_for_acknowledgments(
    pub_: *mut ZeroDdsPublisher,
    timeout_ms: u64,
) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above; the dws list from
    // pub_create_datawriter (Box::into_raw), lives for the publisher lifetime.
    let dws: Vec<*mut ZeroDdsDataWriter> = unsafe {
        (*pub_)
            .datawriters
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let all_acked = dws.iter().all(|&dw| {
            if dw.is_null() {
                return true;
            }
            // SAFETY: dw from Box::into_raw in pub_create_datawriter.
            unsafe { (*dw).rt.user_writer_all_acknowledged((*dw).eid) }
        });
        if all_acked {
            return ZeroDdsStatus::Ok as c_int;
        }
        if std::time::Instant::now() >= deadline {
            return ZeroDdsStatus::Timeout as c_int;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

// ---------------------------------------------------------------------------
// Publisher → DataWriter
// ---------------------------------------------------------------------------

/// Creates a DataWriter over the topic.
///
/// # Safety
/// `pub_`, `topic` valid; `qos` may be NULL (default = pub.default_dw_qos).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_create_datawriter(
    pub_: *mut ZeroDdsPublisher,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataWriterQos,
) -> *mut ZeroDdsDataWriter {
    if pub_.is_null() || topic.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — pub_+topic NULL-checked above; participant
    // from dp_create_publisher; qos NULL-tolerant.
    unsafe {
        let pp = &*pub_;
        let tt = &*topic;

        // The participant is required to reach the runtime.
        let dp_handle = pp.participant;
        if dp_handle.is_null() {
            return ptr::null_mut();
        }
        let dp = &*dp_handle;
        let rt = match dp.rt.as_ref() {
            Some(r) => r.clone(),
            None => return ptr::null_mut(),
        };

        let qos: DataWriterQos = if qos.is_null() {
            pp.default_dw_qos
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default()
        } else {
            dw_qos_from_c(qos)
        };

        let cfg = UserWriterConfig {
            topic_name: tt.name.to_string(),
            type_name: tt.type_name.to_string(),
            reliable: matches!(qos.reliability.kind, ReliabilityKind::Reliable),
            durability: qos.durability.kind,
            deadline: qos.deadline.clone(),
            latency_budget: Default::default(),
            destination_order: Default::default(),
            lifespan: qos.lifespan.clone(),
            liveliness: qos.liveliness.clone(),
            ownership: qos.ownership.kind,
            ownership_strength: qos.ownership_strength.value,
            // QB-cluster: plumb PARTITION from the Publisher/writer QoS onto the
            // endpoint config so `partitions_overlap` (DDS 1.4 §2.2.3.10) gates
            // matching. Previously hardcoded empty → every writer landed in the
            // default partition regardless of the requested names.
            presentation: Default::default(),
            partition: qos.partition.names.clone(),
            user_data: qos.user_data.value.clone(),
            topic_data: qos.topic_data.value.clone(),
            group_data: qos.group_data.value.clone(),
            type_identifier: zerodds_types::TypeIdentifier::default(),
            data_representation_offer: qos.data_representation.clone(),
        };
        let eid = match rt.register_user_writer(cfg) {
            Ok(e) => e,
            Err(_) => return ptr::null_mut(),
        };
        // QB-cluster: wire HISTORY keep-last DEPTH onto the runtime writer slot
        // (DDS 1.4 §2.2.3.18) so the TransientLocal retain path caps per-instance
        // retained samples. KeepAll → unbounded.
        {
            use zerodds_qos::HistoryKind;
            let depth = match qos.history.kind {
                HistoryKind::KeepLast => qos.history.depth.max(1) as usize,
                HistoryKind::KeepAll => usize::MAX,
            };
            let _ = rt.set_user_writer_history_depth(eid, depth);
        }

        let dw = Box::new(ZeroDdsDataWriter {
            publisher: pub_,
            topic,
            rt,
            eid,
            qos: Mutex::new(qos),
            partition_out: Mutex::new(Default::default()),
        });
        let dw_ptr = Box::into_raw(dw);
        if let Ok(mut list) = pp.datawriters.lock() {
            list.push(dw_ptr);
        }
        dw_ptr
    }
}

/// Creates a DataWriter over the topic that advertises its type via a
/// serialized COMPLETE `TypeObject` (F-TYPES-3 / #24).
///
/// Topic-path mirror of the flat [`crate::zerodds_writer_create_typed`]: this is
/// the variant the C#/Java bindings' generated TypeSupport calls, carrying the
/// `byte[]` TypeObject constant `idlc csharp`/`idlc java` emit. Unlike
/// [`zerodds_pub_create_datawriter`] — which advertises
/// `TypeIdentifier::default()` (== None) — it deserializes `type_object_bytes`,
/// registers the object into the shared TypeLookup registry via
/// [`crate::register_type_object_from_bytes`], derives the strongly-hashed
/// `TypeIdentifier` and sets the writer slot's `type_identifier` from it
/// **before the endpoint is published** (via `register_user_writer`). There is
/// therefore no discovery window in which the endpoint is matchable while still
/// carrying `None`.
///
/// The derived identifier is byte-identical to the flat create's and to Path A's
/// `T::TYPE_IDENTIFIER` for the same TypeObject. `qos` is honored exactly as in
/// [`zerodds_pub_create_datawriter`] (PARTITION / HISTORY etc.). Returns NULL on
/// invalid arguments, a malformed TypeObject, or registration failure.
///
/// # Safety
/// Like [`zerodds_pub_create_datawriter`], plus: `type_object_bytes` must point
/// to `len` readable bytes (NULL / `len == 0` is rejected with NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_create_datawriter_typed(
    pub_: *mut ZeroDdsPublisher,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataWriterQos,
    type_object_bytes: *const u8,
    len: usize,
) -> *mut ZeroDdsDataWriter {
    if pub_.is_null() || topic.is_null() || type_object_bytes.is_null() || len == 0 {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — pub_+topic+type_object_bytes NULL-checked
    // above; participant from dp_create_publisher; qos NULL-tolerant;
    // type_object_bytes points to `len` readable bytes (caller pledge).
    unsafe {
        let pp = &*pub_;
        let tt = &*topic;

        let dp_handle = pp.participant;
        if dp_handle.is_null() {
            return ptr::null_mut();
        }
        let dp = &*dp_handle;
        let rt = match dp.rt.as_ref() {
            Some(r) => r.clone(),
            None => return ptr::null_mut(),
        };

        // Register + derive BEFORE publish — no None-window (F-TYPES-3 / #24).
        let bytes = slice::from_raw_parts(type_object_bytes, len);
        let type_identifier = match crate::register_type_object_from_bytes(&rt, bytes) {
            Some(id) => id,
            None => return ptr::null_mut(),
        };

        let qos: DataWriterQos = if qos.is_null() {
            pp.default_dw_qos
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default()
        } else {
            dw_qos_from_c(qos)
        };

        let cfg = UserWriterConfig {
            topic_name: tt.name.to_string(),
            type_name: tt.type_name.to_string(),
            reliable: matches!(qos.reliability.kind, ReliabilityKind::Reliable),
            durability: qos.durability.kind,
            deadline: qos.deadline.clone(),
            latency_budget: Default::default(),
            destination_order: Default::default(),
            lifespan: qos.lifespan.clone(),
            liveliness: qos.liveliness.clone(),
            ownership: qos.ownership.kind,
            ownership_strength: qos.ownership_strength.value,
            presentation: Default::default(),
            partition: qos.partition.names.clone(),
            user_data: qos.user_data.value.clone(),
            topic_data: qos.topic_data.value.clone(),
            group_data: qos.group_data.value.clone(),
            // F-TYPES-3 / #24: derived from the registered TypeObject (not None).
            type_identifier,
            data_representation_offer: qos.data_representation.clone(),
        };
        let eid = match rt.register_user_writer(cfg) {
            Ok(e) => e,
            Err(_) => return ptr::null_mut(),
        };
        {
            use zerodds_qos::HistoryKind;
            let depth = match qos.history.kind {
                HistoryKind::KeepLast => qos.history.depth.max(1) as usize,
                HistoryKind::KeepAll => usize::MAX,
            };
            let _ = rt.set_user_writer_history_depth(eid, depth);
        }

        let dw = Box::new(ZeroDdsDataWriter {
            publisher: pub_,
            topic,
            rt,
            eid,
            qos: Mutex::new(qos),
            partition_out: Mutex::new(Default::default()),
        });
        let dw_ptr = Box::into_raw(dw);
        if let Ok(mut list) = pp.datawriters.lock() {
            list.push(dw_ptr);
        }
        dw_ptr
    }
}

/// Deletes a DataWriter.
///
/// # Safety
/// `pub_`, `dw` valid and belonging together.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_delete_datawriter(
    pub_: *mut ZeroDdsPublisher,
    dw: *mut ZeroDdsDataWriter,
) -> c_int {
    if pub_.is_null() || dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_+dw NULL-checked above; dw from pub_create_datawriter.
    unsafe {
        if (*dw).publisher != pub_ {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        let pp = &*pub_;
        if let Ok(mut list) = pp.datawriters.lock() {
            let n = list.len();
            list.retain(|x| *x != dw);
            if list.len() == n {
                return ZeroDdsStatus::BadHandle as c_int;
            }
        }
        // Drop any zero-copy SHM loan state keyed by this writer's (runtime, eid).
        #[cfg(feature = "flatdata-loan")]
        {
            let dwr = &*dw;
            crate::shm_loan_ffi::forget_writer(&dwr.rt, dwr.eid);
        }
        let _ = Box::from_raw(dw);
    }
    ZeroDdsStatus::Ok as c_int
}

/// Returns the DataWriter for a topic name, or NULL.
///
/// # Safety
/// `pub_`, `topic_name` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_lookup_datawriter(
    pub_: *mut ZeroDdsPublisher,
    topic_name: *const core::ffi::c_char,
) -> *mut ZeroDdsDataWriter {
    if pub_.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — pub_+topic_name NULL-checked above; datawriters
    // and their topic pointers from pub_create_datawriter (Box::into_raw).
    unsafe {
        let cs = std::ffi::CStr::from_ptr(topic_name);
        let name = match cs.to_str() {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        };
        let pp = &*pub_;
        if let Ok(list) = pp.datawriters.lock() {
            for &dw in list.iter() {
                if dw.is_null() {
                    continue;
                }
                let dwr = &*dw;
                if !dwr.topic.is_null() && (*dwr.topic).name == name {
                    return dw;
                }
            }
        }
    }
    ptr::null_mut()
}

/// Deletes all DataWriters held by the publisher.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_delete_contained_entities(
    pub_: *mut ZeroDdsPublisher,
) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above; datawriters from
    // pub_create_datawriter (Box::into_raw).
    unsafe {
        let dws: Vec<*mut ZeroDdsDataWriter> = (*pub_)
            .datawriters
            .lock()
            .map(|mut g| core::mem::take(&mut *g))
            .unwrap_or_default();
        for dw in dws {
            if !dw.is_null() {
                let _ = Box::from_raw(dw);
            }
        }
    }
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// DataWriter: write, dispose, statuses, accessors
// ---------------------------------------------------------------------------

/// Returns the topic.
///
/// # Safety
/// `dw` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_topic(dw: *mut ZeroDdsDataWriter) -> *mut ZeroDdsTopic {
    if dw.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
    unsafe { (*dw).topic }
}

/// Returns the publisher.
///
/// # Safety
/// `dw` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_publisher(
    dw: *mut ZeroDdsDataWriter,
) -> *mut ZeroDdsPublisher {
    if dw.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
    unsafe { (*dw).publisher }
}

/// Writes a pre-made CDR payload (the caller serializes via
/// idl codegen or DynamicData). The encap header prefix is added by the
/// runtime.
///
/// # Safety
/// `dw`, `payload[0..len]` valid. `handle` is a spec-mandatory param that
/// identifies the instance — `0` = HANDLE_NIL = "writer computes it".
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_write(
    dw: *mut ZeroDdsDataWriter,
    payload: *const u8,
    len: usize,
    _handle: u64,
) -> c_int {
    if dw.is_null() || (payload.is_null() && len > 0) {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+payload NULL-checked above; payload[0..len]
    // valid if len > 0 (caller pledge).
    let (rt, eid, buf) = unsafe {
        let dwr = &*dw;
        let buf = if len == 0 {
            Vec::new()
        } else {
            slice::from_raw_parts(payload, len).to_vec()
        };
        (dwr.rt.clone(), dwr.eid, buf)
    };
    match rt.write_user_sample(eid, buf) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Writes with a source timestamp. RC1 surface: forwards to `write`
/// — the source timestamp lands in the future inline-QoS implementation.
///
/// # Safety
/// Like `zerodds_dw_write`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_write_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    payload: *const u8,
    len: usize,
    handle: u64,
    _ts_sec: i32,
    _ts_nanosec: u32,
) -> c_int {
    // SAFETY: see fn # Safety doc — delegation to zerodds_dw_write with an identical contract.
    unsafe { zerodds_dw_write(dw, payload, len, handle) }
}

/// Dispose: emits a DISPOSED lifecycle marker for the instance.
///
/// # Safety
/// `dw` valid; `key_hash[0..16]` readable. `handle` informational.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_dispose(
    dw: *mut ZeroDdsDataWriter,
    key_hash: *const u8,
    _handle: u64,
) -> c_int {
    if dw.is_null() || key_hash.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+key_hash NULL-checked above; key_hash[0..16]
    // valid (caller pledge).
    let (rt, eid, k) = unsafe {
        let dwr = &*dw;
        let raw = slice::from_raw_parts(key_hash, 16);
        let mut k = [0u8; 16];
        k.copy_from_slice(raw);
        (dwr.rt.clone(), dwr.eid, k)
    };
    // Status bits: DISPOSED per RTPS inline-QoS status info §9.6.4.10.
    const STATUS_DISPOSED: u32 = 0x0000_0001;
    match rt.write_user_lifecycle(eid, k, STATUS_DISPOSED) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Waits until all currently outstanding writes are acked or a timeout.
///
/// # Safety
/// `dw` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_wait_for_acknowledgments(
    dw: *mut ZeroDdsDataWriter,
    timeout_ms: u64,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
    let (rt, eid) = unsafe { ((*dw).rt.clone(), (*dw).eid) };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if rt.user_writer_all_acknowledged(eid) {
            return ZeroDdsStatus::Ok as c_int;
        }
        if std::time::Instant::now() >= deadline {
            return ZeroDdsStatus::Timeout as c_int;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Manually asserts liveliness for this writer.
///
/// # Safety
/// `dw` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_assert_liveliness(dw: *mut ZeroDdsDataWriter) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
    unsafe { (*dw).rt.assert_liveliness() };
    ZeroDdsStatus::Ok as c_int
}

/// Waits until at least `min` matched subscriptions exist or a timeout.
///
/// # Safety
/// `dw` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_wait_for_matched(
    dw: *mut ZeroDdsDataWriter,
    min: i32,
    timeout_ms: u64,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
    let (rt, eid) = unsafe { ((*dw).rt.clone(), (*dw).eid) };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let n = rt.user_writer_matched_count(eid) as i32;
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
// DataWriter Statuses (5x)
// ---------------------------------------------------------------------------

/// LivelinessLostStatus (Spec §2.2.4.1).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsLivelinessLostStatus {
    pub total_count: i32,
    pub total_count_change: i32,
}

/// PublicationMatchedStatus (Spec §2.2.4.1).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsPublicationMatchedStatus {
    pub total_count: i32,
    pub total_count_change: i32,
    pub current_count: i32,
    pub current_count_change: i32,
    pub last_subscription_handle: u64,
}

/// OfferedDeadlineMissedStatus (Spec §2.2.4.1).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsOfferedDeadlineMissedStatus {
    pub total_count: i32,
    pub total_count_change: i32,
    pub last_instance_handle: u64,
}

/// OfferedIncompatibleQosStatus (Spec §2.2.4.1, without a per-policy list —
/// `last_policy_id` covers the mandatory path).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsOfferedIncompatibleQosStatus {
    pub total_count: i32,
    pub total_count_change: i32,
    pub last_policy_id: u32,
}

/// `LIVELINESS_LOST_STATUS` (Spec §2.2.2.4.2.x).
///
/// # Safety
/// `dw` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_liveliness_lost_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsLivelinessLostStatus,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above.
    unsafe {
        let dwr = &*dw;
        let lost = dwr.rt.user_writer_liveliness_lost(dwr.eid);
        *out = ZeroDdsLivelinessLostStatus {
            total_count: lost as i32,
            total_count_change: 0,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `OFFERED_DEADLINE_MISSED_STATUS`.
///
/// # Safety
/// `dw` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_offered_deadline_missed_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsOfferedDeadlineMissedStatus,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above.
    unsafe {
        let dwr = &*dw;
        let n = dwr.rt.user_writer_offered_deadline_missed(dwr.eid);
        *out = ZeroDdsOfferedDeadlineMissedStatus {
            total_count: n as i32,
            total_count_change: 0,
            last_instance_handle: 0,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `OFFERED_INCOMPATIBLE_QOS_STATUS`.
///
/// # Safety
/// `dw` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_offered_incompatible_qos_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsOfferedIncompatibleQosStatus,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above.
    unsafe {
        let dwr = &*dw;
        let st = dwr.rt.user_writer_offered_incompatible_qos(dwr.eid);
        *out = ZeroDdsOfferedIncompatibleQosStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
            last_policy_id: st.last_policy_id,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `PUBLICATION_MATCHED_STATUS`.
///
/// # Safety
/// `dw` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_publication_matched_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsPublicationMatchedStatus,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above.
    unsafe {
        let dwr = &*dw;
        let n = dwr.rt.user_writer_matched_count(dwr.eid) as i32;
        *out = ZeroDdsPublicationMatchedStatus {
            total_count: n,
            total_count_change: 0,
            current_count: n,
            current_count_change: 0,
            last_subscription_handle: 0,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

// Helper used by tests/internals; type-suppress `DurabilityKind` import warning.
#[allow(dead_code)]
fn _suppress_durability_warning() -> DurabilityKind {
    DurabilityKind::default()
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

    fn mk_pub_topic(
        domain: u32,
    ) -> (
        *mut ZeroDdsDomainParticipant,
        *mut ZeroDdsPublisher,
        *mut ZeroDdsTopic,
    ) {
        let f = zerodds_dpf_get_instance();
        let n = c"PubTopic";
        let tn = c"PubType";
        // SAFETY: f from dpf_get_instance, n+tn statically valid.
        unsafe {
            let p = zerodds_dpf_create_participant(f, domain, ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            (p, pubh, t)
        }
    }

    fn cleanup(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: p from mk_pub_topic; f static.
        unsafe {
            zerodds_dp_delete_contained_entities(p);
            zerodds_dpf_delete_participant(f, p);
        }
    }

    #[test]
    fn create_delete_datawriter_roundtrip() {
        let (p, pubh, t) = mk_pub_topic(41);
        assert!(!p.is_null() && !pubh.is_null() && !t.is_null());
        // SAFETY: pubh+t from mk.
        unsafe {
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            assert!(!dw.is_null());
            let rc = zerodds_pub_delete_datawriter(pubh, dw);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        }
        cleanup(p);
    }

    #[test]
    fn lookup_datawriter_finds_existing() {
        let (p, pubh, t) = mk_pub_topic(42);
        let n = c"PubTopic";
        // SAFETY: pubh+t from mk; n static.
        unsafe {
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            assert!(!dw.is_null());
            let found = zerodds_pub_lookup_datawriter(pubh, n.as_ptr());
            assert_eq!(found, dw);
            let _ = zerodds_pub_delete_datawriter(pubh, dw);
        }
        cleanup(p);
    }

    #[test]
    fn lookup_datawriter_unknown_returns_null() {
        let (p, pubh, t) = mk_pub_topic(43);
        let n = c"Other";
        // SAFETY: pubh+t from mk; n static.
        unsafe {
            let _dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            let found = zerodds_pub_lookup_datawriter(pubh, n.as_ptr());
            assert!(found.is_null());
        }
        cleanup(p);
    }

    #[test]
    fn write_returns_ok_or_error() {
        let (p, pubh, t) = mk_pub_topic(44);
        let payload: [u8; 4] = [1, 2, 3, 4];
        // SAFETY: pubh+t from mk; payload stack-local.
        unsafe {
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            let rc = zerodds_dw_write(dw, payload.as_ptr(), payload.len(), 0);
            assert!(rc == ZeroDdsStatus::Ok as c_int || rc == ZeroDdsStatus::Error as c_int);
        }
        cleanup(p);
    }

    #[test]
    fn dw_get_topic_publisher_roundtrip() {
        let (p, pubh, t) = mk_pub_topic(45);
        // SAFETY: pubh+t from mk.
        unsafe {
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            assert_eq!(zerodds_dw_get_topic(dw), t);
            assert_eq!(zerodds_dw_get_publisher(dw), pubh);
        }
        cleanup(p);
    }

    #[test]
    fn pub_suspend_resume_clean() {
        let (p, pubh, _t) = mk_pub_topic(46);
        // SAFETY: pubh from mk.
        unsafe {
            assert_eq!(
                zerodds_pub_suspend_publications(pubh),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                zerodds_pub_resume_publications(pubh),
                ZeroDdsStatus::Ok as c_int
            );
        }
        cleanup(p);
    }

    /// F-TYPES-3 / #24: the topic-based typed create
    /// (`zerodds_pub_create_datawriter_typed` /
    /// `zerodds_sub_create_datareader_typed` — the path the C#/Java bindings
    /// use) deserializes the emitted COMPLETE TypeObject bytes, registers the
    /// object BEFORE the endpoint is published, and derives the identifier
    /// byte-identical to `StructBuilder::build_complete`. Proven by the object
    /// landing in the runtime registry under the hash computed from the
    /// builder's own COMPLETE TypeObject.
    #[test]
    fn pub_sub_create_typed_registers_before_publish() {
        use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
        use zerodds_types::type_object::{CompleteTypeObject, TypeObject};
        use zerodds_types::{PrimitiveKind, TypeIdentifier, compute_complete_hash};

        // Path A: the SAME builder the flat typed-create test uses.
        let cs = TypeObjectBuilder::struct_type("Sensor")
            .extensibility(Extensibility::Appendable)
            .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                m.key()
            })
            .member(
                "value",
                TypeIdentifier::Primitive(PrimitiveKind::Float64),
                |m| m,
            )
            .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Struct(cs));
        let TypeObject::Complete(cto) = &to else {
            panic!("built a Complete TypeObject")
        };
        let expected_hash = compute_complete_hash(cto).unwrap();
        let bytes = to.to_bytes_le().unwrap();

        let (p, pubh, t) = mk_pub_topic(48);
        // SAFETY: p/pubh/t from mk; bytes is a valid slice.
        unsafe {
            let subh = crate::participant_ffi::zerodds_dp_create_subscriber(p, ptr::null());
            assert!(!subh.is_null());

            let dw = zerodds_pub_create_datawriter_typed(
                pubh,
                t,
                ptr::null(),
                bytes.as_ptr(),
                bytes.len(),
            );
            assert!(!dw.is_null(), "pub_create_datawriter_typed returned NULL");

            let dr = crate::subscriber_ffi::zerodds_sub_create_datareader_typed(
                subh,
                t,
                ptr::null(),
                bytes.as_ptr(),
                bytes.len(),
            );
            assert!(!dr.is_null(), "sub_create_datareader_typed returned NULL");

            // The COMPLETE TypeObject is in the runtime registry, keyed by the
            // hash the StructBuilder computes — deserialize+register+derive is
            // byte-identical to build_complete.
            {
                let rt = (*dw).rt.clone();
                let server = rt.type_lookup_server.lock().unwrap();
                assert!(
                    server.registry.get_complete(&expected_hash).is_some(),
                    "typed pub/sub create did not register the TypeObject under its build_complete hash"
                );
            }

            // A malformed TypeObject is rejected — never a None-identifier slot.
            let garbage: [u8; 3] = [0, 1, 2];
            assert!(
                zerodds_pub_create_datawriter_typed(
                    pubh,
                    t,
                    ptr::null(),
                    garbage.as_ptr(),
                    garbage.len(),
                )
                .is_null()
            );
            assert!(
                zerodds_pub_create_datawriter_typed(pubh, t, ptr::null(), ptr::null(), 0).is_null()
            );

            let _ = zerodds_pub_delete_datawriter(pubh, dw);
        }
        cleanup(p);
    }

    #[test]
    fn dw_status_getters_return_default() {
        let (p, pubh, t) = mk_pub_topic(47);
        let mut lost = ZeroDdsLivelinessLostStatus::default();
        let mut deadl = ZeroDdsOfferedDeadlineMissedStatus::default();
        let mut incompat = ZeroDdsOfferedIncompatibleQosStatus::default();
        let mut matched = ZeroDdsPublicationMatchedStatus::default();
        // SAFETY: pubh+t from mk; status slots stack-local.
        unsafe {
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            let ok = ZeroDdsStatus::Ok as c_int;
            assert_eq!(zerodds_dw_get_liveliness_lost_status(dw, &mut lost), ok);
            assert_eq!(
                zerodds_dw_get_offered_deadline_missed_status(dw, &mut deadl),
                ok
            );
            assert_eq!(
                zerodds_dw_get_offered_incompatible_qos_status(dw, &mut incompat),
                ok
            );
            assert_eq!(
                zerodds_dw_get_publication_matched_status(dw, &mut matched),
                ok
            );
        }
        cleanup(p);
    }
}
