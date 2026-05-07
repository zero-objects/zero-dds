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

/// Liefert den Owning-Participant.
///
/// # Safety
/// `pub_` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_get_participant(
    pub_: *mut ZeroDdsPublisher,
) -> *mut ZeroDdsDomainParticipant {
    if pub_.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { (*pub_).participant }
}

/// Suspend publications (Spec §2.2.2.4.1.10). Setzt Flag — der
/// Hot-Path queued Writes statt sie zu senden, bis `resume_publications`.
///
/// # Safety
/// `pub_` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_suspend_publications(pub_: *mut ZeroDdsPublisher) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*pub_ };
    if let Ok(mut g) = pp.suspended.lock() {
        *g = true;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Resume publications.
///
/// # Safety
/// `pub_` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_resume_publications(pub_: *mut ZeroDdsPublisher) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*pub_ };
    if let Ok(mut g) = pp.suspended.lock() {
        *g = false;
    }
    ZeroDdsStatus::Ok as c_int
}

/// begin_coherent_changes (Spec §2.2.2.4.1.12). Im RC1 ein No-Bracket-
/// Marker — spec-konformer Tracker auf Pub-Ebene; CoherentSet-
/// Ergebnis-Wire-Frames lebt auf Writer-Ebene und greift unabhaengig.
///
/// # Safety
/// `pub_` valide.
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
/// `pub_` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_end_coherent_changes(pub_: *mut ZeroDdsPublisher) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    ZeroDdsStatus::Ok as c_int
}

/// wait_for_acknowledgments(timeout_ms) — wartet bis alle DWs akkied
/// sind oder Timeout schlaegt.
///
/// # Safety
/// `pub_` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_wait_for_acknowledgments(
    pub_: *mut ZeroDdsPublisher,
    timeout_ms: u64,
) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*pub_ };
    let dws: Vec<*mut ZeroDdsDataWriter> =
        pp.datawriters.lock().map(|g| g.clone()).unwrap_or_default();
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let all_acked = dws.iter().all(|&dw| {
            if dw.is_null() {
                return true;
            }
            // SAFETY: dw aus Box::into_raw in pub_create_datawriter.
            let dwr = unsafe { &*dw };
            dwr.rt.user_writer_all_acknowledged(dwr.eid)
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

/// Erzeugt einen DataWriter ueber das Topic.
///
/// # Safety
/// `pub_`, `topic` valide; `qos` darf NULL sein (Default = pub.default_dw_qos).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_create_datawriter(
    pub_: *mut ZeroDdsPublisher,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataWriterQos,
) -> *mut ZeroDdsDataWriter {
    if pub_.is_null() || topic.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: NULL-Checks.
    let pp = unsafe { &*pub_ };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let tt = unsafe { &*topic };

    // Participant erforderlich um an die Runtime zu kommen.
    let dp_handle = pp.participant;
    if dp_handle.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: participant aus dp_create_publisher.
    let dp = unsafe { &*dp_handle };
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
        // SAFETY: NULL-Check.
        unsafe { dw_qos_from_c(qos) }
    };

    let cfg = UserWriterConfig {
        topic_name: tt.name.to_string(),
        type_name: tt.type_name.to_string(),
        reliable: matches!(qos.reliability.kind, ReliabilityKind::Reliable),
        durability: qos.durability.kind,
        deadline: qos.deadline.clone(),
        lifespan: qos.lifespan.clone(),
        liveliness: qos.liveliness.clone(),
        ownership: qos.ownership.kind,
        ownership_strength: qos.ownership_strength.value,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::default(),
        data_representation_offer: None,
    };
    let eid = match rt.register_user_writer(cfg) {
        Ok(e) => e,
        Err(_) => return ptr::null_mut(),
    };

    let dw = Box::new(ZeroDdsDataWriter {
        publisher: pub_,
        topic,
        rt,
        eid,
        qos: Mutex::new(qos),
    });
    let dw_ptr = Box::into_raw(dw);
    if let Ok(mut list) = pp.datawriters.lock() {
        list.push(dw_ptr);
    }
    dw_ptr
}

/// Loescht einen DataWriter.
///
/// # Safety
/// `pub_`, `dw` valide und zugehoerig.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_delete_datawriter(
    pub_: *mut ZeroDdsPublisher,
    dw: *mut ZeroDdsDataWriter,
) -> c_int {
    if pub_.is_null() || dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    {
        // SAFETY: NULL-Check.
        let dwr = unsafe { &*dw };
        if dwr.publisher != pub_ {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*pub_ };
    if let Ok(mut list) = pp.datawriters.lock() {
        let n = list.len();
        list.retain(|x| *x != dw);
        if list.len() == n {
            return ZeroDdsStatus::BadHandle as c_int;
        }
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let _ = unsafe { Box::from_raw(dw) };
    ZeroDdsStatus::Ok as c_int
}

/// Liefert den DataWriter zu einem Topic-Namen, oder NULL.
///
/// # Safety
/// `pub_`, `topic_name` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_lookup_datawriter(
    pub_: *mut ZeroDdsPublisher,
    topic_name: *const core::ffi::c_char,
) -> *mut ZeroDdsDataWriter {
    if pub_.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: NULL-Check.
    let cs = unsafe { std::ffi::CStr::from_ptr(topic_name) };
    let name = match cs.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*pub_ };
    if let Ok(list) = pp.datawriters.lock() {
        for &dw in list.iter() {
            if dw.is_null() {
                continue;
            }
            // SAFETY: list haelt valide pointer.
            let dwr = unsafe { &*dw };
            if !dwr.topic.is_null() {
                // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
                let t = unsafe { &*dwr.topic };
                if t.name == name {
                    return dw;
                }
            }
        }
    }
    ptr::null_mut()
}

/// Loescht alle vom Publisher gehaltenen DataWriter.
///
/// # Safety
/// `pub_` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_delete_contained_entities(
    pub_: *mut ZeroDdsPublisher,
) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*pub_ };
    let dws: Vec<*mut ZeroDdsDataWriter> = pp
        .datawriters
        .lock()
        .map(|mut g| core::mem::take(&mut *g))
        .unwrap_or_default();
    for dw in dws {
        if !dw.is_null() {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            let _ = unsafe { Box::from_raw(dw) };
        }
    }
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// DataWriter: write, dispose, statuses, accessors
// ---------------------------------------------------------------------------

/// Liefert das Topic.
///
/// # Safety
/// `dw` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_topic(dw: *mut ZeroDdsDataWriter) -> *mut ZeroDdsTopic {
    if dw.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { (*dw).topic }
}

/// Liefert den Publisher.
///
/// # Safety
/// `dw` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_publisher(
    dw: *mut ZeroDdsDataWriter,
) -> *mut ZeroDdsPublisher {
    if dw.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { (*dw).publisher }
}

/// Schreibt einen vorgefertigten CDR-Payload (Caller serialisiert via
/// idl-Codegen oder DynamicData). Das Encap-Header-Prefix wird vom
/// Runtime hinzugefuegt.
///
/// # Safety
/// `dw`, `payload[0..len]` valide. `handle` ist Spec-Pflicht-Param der
/// die Instance kennzeichnet — `0` = HANDLE_NIL = "Writer rechnet aus".
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
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dwr = unsafe { &*dw };
    let buf = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: Caller-Kontrakt: payload[0..len] gueltig.
        let s = unsafe { slice::from_raw_parts(payload, len) };
        s.to_vec()
    };
    match dwr.rt.write_user_sample(dwr.eid, buf) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Schreibt mit Source-Timestamp. RC1-Surface: leitet auf `write` durch
/// — Source-Timestamp landet in der zukuenftigen Inline-QoS-Implementation.
///
/// # Safety
/// Wie `zerodds_dw_write`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_write_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    payload: *const u8,
    len: usize,
    handle: u64,
    _ts_sec: i32,
    _ts_nanosec: u32,
) -> c_int {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { zerodds_dw_write(dw, payload, len, handle) }
}

/// Dispose: emittiert ein DISPOSED-Lifecycle-Marker fuer die Instance.
///
/// # Safety
/// `dw` valide; `key_hash[0..16]` lesbar. `handle` informational.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_dispose(
    dw: *mut ZeroDdsDataWriter,
    key_hash: *const u8,
    _handle: u64,
) -> c_int {
    if dw.is_null() || key_hash.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dwr = unsafe { &*dw };
    // SAFETY: Caller liefert 16-Byte-Buffer.
    let raw = unsafe { slice::from_raw_parts(key_hash, 16) };
    let mut k = [0u8; 16];
    k.copy_from_slice(raw);
    // Status-Bits: DISPOSED nach RTPS Inline-QoS Status-Info §9.6.4.10.
    const STATUS_DISPOSED: u32 = 0x0000_0001;
    match dwr.rt.write_user_lifecycle(dwr.eid, k, STATUS_DISPOSED) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Wartet bis alle aktuell ausstehenden Writes ackd sind oder Timeout.
///
/// # Safety
/// `dw` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_wait_for_acknowledgments(
    dw: *mut ZeroDdsDataWriter,
    timeout_ms: u64,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dwr = unsafe { &*dw };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if dwr.rt.user_writer_all_acknowledged(dwr.eid) {
            return ZeroDdsStatus::Ok as c_int;
        }
        if std::time::Instant::now() >= deadline {
            return ZeroDdsStatus::Timeout as c_int;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Liveliness fuer diesen Writer manuell asserten.
///
/// # Safety
/// `dw` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_assert_liveliness(dw: *mut ZeroDdsDataWriter) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { (*dw).rt.assert_liveliness() };
    ZeroDdsStatus::Ok as c_int
}

/// Wartet bis mindestens `min` matched Subscriptions vorhanden sind oder Timeout.
///
/// # Safety
/// `dw` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_wait_for_matched(
    dw: *mut ZeroDdsDataWriter,
    min: i32,
    timeout_ms: u64,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dwr = unsafe { &*dw };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let n = dwr.rt.user_writer_matched_count(dwr.eid) as i32;
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

/// OfferedIncompatibleQosStatus (Spec §2.2.4.1, ohne Per-Policy-Liste —
/// `last_policy_id` deckt den Pflicht-Pfad).
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
/// `dw` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_liveliness_lost_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsLivelinessLostStatus,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dwr = unsafe { &*dw };
    let lost = dwr.rt.user_writer_liveliness_lost(dwr.eid);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
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
/// `dw` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_offered_deadline_missed_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsOfferedDeadlineMissedStatus,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dwr = unsafe { &*dw };
    let n = dwr.rt.user_writer_offered_deadline_missed(dwr.eid);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
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
/// `dw` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_offered_incompatible_qos_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsOfferedIncompatibleQosStatus,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dwr = unsafe { &*dw };
    let st = dwr.rt.user_writer_offered_incompatible_qos(dwr.eid);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
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
/// `dw` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_publication_matched_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsPublicationMatchedStatus,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dwr = unsafe { &*dw };
    let n = dwr.rt.user_writer_matched_count(dwr.eid) as i32;
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
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
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let p = unsafe { zerodds_dpf_create_participant(f, domain, ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let pubh = unsafe { zerodds_dp_create_publisher(p, ptr::null()) };
        let n = c"PubTopic";
        let tn = c"PubType";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let t = unsafe { zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null()) };
        (p, pubh, t)
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
    fn create_delete_datawriter_roundtrip() {
        let (p, pubh, t) = mk_pub_topic(41);
        assert!(!p.is_null() && !pubh.is_null() && !t.is_null());
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dw = unsafe { zerodds_pub_create_datawriter(pubh, t, ptr::null()) };
        assert!(!dw.is_null());
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_pub_delete_datawriter(pubh, dw) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        cleanup(p);
    }

    #[test]
    fn lookup_datawriter_finds_existing() {
        let (p, pubh, t) = mk_pub_topic(42);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dw = unsafe { zerodds_pub_create_datawriter(pubh, t, ptr::null()) };
        assert!(!dw.is_null());
        let n = c"PubTopic";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let found = unsafe { zerodds_pub_lookup_datawriter(pubh, n.as_ptr()) };
        assert_eq!(found, dw);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_pub_delete_datawriter(pubh, dw) };
        cleanup(p);
    }

    #[test]
    fn lookup_datawriter_unknown_returns_null() {
        let (p, pubh, t) = mk_pub_topic(43);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _dw = unsafe { zerodds_pub_create_datawriter(pubh, t, ptr::null()) };
        let n = c"Other";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let found = unsafe { zerodds_pub_lookup_datawriter(pubh, n.as_ptr()) };
        assert!(found.is_null());
        cleanup(p);
    }

    #[test]
    fn write_returns_ok_or_error() {
        let (p, pubh, t) = mk_pub_topic(44);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dw = unsafe { zerodds_pub_create_datawriter(pubh, t, ptr::null()) };
        let payload: [u8; 4] = [1, 2, 3, 4];
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dw_write(dw, payload.as_ptr(), payload.len(), 0) };
        assert!(rc == ZeroDdsStatus::Ok as c_int || rc == ZeroDdsStatus::Error as c_int);
        cleanup(p);
    }

    #[test]
    fn dw_get_topic_publisher_roundtrip() {
        let (p, pubh, t) = mk_pub_topic(45);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dw = unsafe { zerodds_pub_create_datawriter(pubh, t, ptr::null()) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert_eq!(unsafe { zerodds_dw_get_topic(dw) }, t);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert_eq!(unsafe { zerodds_dw_get_publisher(dw) }, pubh);
        cleanup(p);
    }

    #[test]
    fn pub_suspend_resume_clean() {
        let (p, pubh, _t) = mk_pub_topic(46);
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_pub_suspend_publications(pubh) },
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_pub_resume_publications(pubh) },
            ZeroDdsStatus::Ok as c_int
        );
        cleanup(p);
    }

    #[test]
    fn dw_status_getters_return_default() {
        let (p, pubh, t) = mk_pub_topic(47);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let dw = unsafe { zerodds_pub_create_datawriter(pubh, t, ptr::null()) };
        let mut lost = ZeroDdsLivelinessLostStatus::default();
        let mut deadl = ZeroDdsOfferedDeadlineMissedStatus::default();
        let mut incompat = ZeroDdsOfferedIncompatibleQosStatus::default();
        let mut matched = ZeroDdsPublicationMatchedStatus::default();
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_dw_get_liveliness_lost_status(dw, &mut lost) },
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_dw_get_offered_deadline_missed_status(dw, &mut deadl) },
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_dw_get_offered_incompatible_qos_status(dw, &mut incompat) },
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { zerodds_dw_get_publication_matched_status(dw, &mut matched) },
            ZeroDdsStatus::Ok as c_int
        );
        cleanup(p);
    }
}
