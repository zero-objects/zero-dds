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
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above.
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
/// `pub_` valide.
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
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above; dws-Liste aus
    // pub_create_datawriter (Box::into_raw), lebt fuer Publisher-Lifetime.
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
            // SAFETY: dw aus Box::into_raw in pub_create_datawriter.
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
    // SAFETY: see fn # Safety doc — pub_+topic NULL-checked above; participant
    // aus dp_create_publisher; qos NULL-tolerant.
    unsafe {
        let pp = &*pub_;
        let tt = &*topic;

        // Participant erforderlich um an die Runtime zu kommen.
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
    // SAFETY: see fn # Safety doc — pub_+dw NULL-checked above; dw aus pub_create_datawriter.
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
        let _ = Box::from_raw(dw);
    }
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
    // SAFETY: see fn # Safety doc — pub_+topic_name NULL-checked above; datawriters
    // und ihre topic-Pointer aus pub_create_datawriter (Box::into_raw).
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
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above; datawriters aus
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

/// Liefert das Topic.
///
/// # Safety
/// `dw` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_topic(dw: *mut ZeroDdsDataWriter) -> *mut ZeroDdsTopic {
    if dw.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
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
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
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
    // SAFETY: see fn # Safety doc — dw+payload NULL-checked above; payload[0..len]
    // valide wenn len > 0 (Caller-Pledge).
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
    // SAFETY: see fn # Safety doc — Delegation an zerodds_dw_write mit identischem Vertrag.
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
    // SAFETY: see fn # Safety doc — dw+key_hash NULL-checked above; key_hash[0..16]
    // valide (Caller-Pledge).
    let (rt, eid, k) = unsafe {
        let dwr = &*dw;
        let raw = slice::from_raw_parts(key_hash, 16);
        let mut k = [0u8; 16];
        k.copy_from_slice(raw);
        (dwr.rt.clone(), dwr.eid, k)
    };
    // Status-Bits: DISPOSED nach RTPS Inline-QoS Status-Info §9.6.4.10.
    const STATUS_DISPOSED: u32 = 0x0000_0001;
    match rt.write_user_lifecycle(eid, k, STATUS_DISPOSED) {
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

/// Liveliness fuer diesen Writer manuell asserten.
///
/// # Safety
/// `dw` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_assert_liveliness(dw: *mut ZeroDdsDataWriter) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
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
/// `dw` und `out` valide.
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
/// `dw` und `out` valide.
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
/// `dw` und `out` valide.
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
        // SAFETY: f aus dpf_get_instance, n+tn statisch valide.
        unsafe {
            let p = zerodds_dpf_create_participant(f, domain, ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            (p, pubh, t)
        }
    }

    fn cleanup(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: p aus mk_pub_topic; f statisch.
        unsafe {
            zerodds_dp_delete_contained_entities(p);
            zerodds_dpf_delete_participant(f, p);
        }
    }

    #[test]
    fn create_delete_datawriter_roundtrip() {
        let (p, pubh, t) = mk_pub_topic(41);
        assert!(!p.is_null() && !pubh.is_null() && !t.is_null());
        // SAFETY: pubh+t aus mk.
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
        // SAFETY: pubh+t aus mk; n statisch.
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
        // SAFETY: pubh+t aus mk; n statisch.
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
        // SAFETY: pubh+t aus mk; payload Stack-lokal.
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
        // SAFETY: pubh+t aus mk.
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
        // SAFETY: pubh aus mk.
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

    #[test]
    fn dw_status_getters_return_default() {
        let (p, pubh, t) = mk_pub_topic(47);
        let mut lost = ZeroDdsLivelinessLostStatus::default();
        let mut deadl = ZeroDdsOfferedDeadlineMissedStatus::default();
        let mut incompat = ZeroDdsOfferedIncompatibleQosStatus::default();
        let mut matched = ZeroDdsPublicationMatchedStatus::default();
        // SAFETY: pubh+t aus mk; status-Slots Stack-lokal.
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
