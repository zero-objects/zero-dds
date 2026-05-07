// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Topic C-FFI (Spec §2.2.2.3.1 + DDS-PSM-Cxx §7.5.4).

use alloc::ffi::CString;
use core::ffi::{c_char, c_int};
use core::ptr;

use crate::ZeroDdsStatus;
use crate::entities::{ZeroDdsDomainParticipant, ZeroDdsTopic};

/// Liefert den Topic-Namen als heap-allokierten C-String. Caller muss
/// `zerodds_string_free` rufen.
///
/// # Safety
/// `t` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_name(t: *mut ZeroDdsTopic) -> *mut c_char {
    if t.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: NULL-Check.
    let tt = unsafe { &*t };
    match CString::new(tt.name.as_str()) {
        Ok(c) => c.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Liefert den Topic-Type-Namen. Caller muss `zerodds_string_free` rufen.
///
/// # Safety
/// `t` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_type_name(t: *mut ZeroDdsTopic) -> *mut c_char {
    if t.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let tt = unsafe { &*t };
    match CString::new(tt.type_name.as_str()) {
        Ok(c) => c.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Liefert den Owning-Participant.
///
/// # Safety
/// `t` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_participant(
    t: *mut ZeroDdsTopic,
) -> *mut ZeroDdsDomainParticipant {
    if t.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { (*t).participant }
}

/// Gibt einen vorher von `zerodds_topic_get_*` allokierten C-String frei.
///
/// # Safety
/// `s` muss aus einer `zerodds_*_get_*`-Funktion stammen, die einen
/// `*mut c_char` per `CString::into_raw` produziert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    // SAFETY: Caller-Kontrakt: s stammt aus CString::into_raw.
    let _ = unsafe { CString::from_raw(s) };
}

/// Get-QoS in `out` (Spec §2.2.2.3.2.x).
/// `out` ist `*mut ZeroDdsTopicQos` mit Caller-supplied Buffer fuer
/// `topic_data.value` (variable Laenge).
///
/// # Safety
/// `t`, `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_qos(
    t: *mut ZeroDdsTopic,
    out: *mut crate::qos_ffi::ZeroDdsTopicQos,
) -> c_int {
    if t.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let tt = unsafe { &*t };
    let qos = tt.qos.lock().map(|g| g.clone()).unwrap_or_default();
    // SAFETY: NULL-Check; topic_qos_to_c handhabt variable-Length-Felder.
    unsafe { crate::qos_ffi::topic_qos_to_c(&qos, out) }
}

/// Set-QoS (Spec §2.2.2.3.2.x). NULL = Default.
///
/// # Safety
/// `t` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_set_qos(
    t: *mut ZeroDdsTopic,
    qos: *const crate::qos_ffi::ZeroDdsTopicQos,
) -> c_int {
    if t.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let tt = unsafe { &*t };
    let new_qos = if qos.is_null() {
        zerodds_dcps::qos::TopicQos::default()
    } else {
        // SAFETY: Caller-Kontrakt.
        unsafe { crate::qos_ffi::topic_qos_from_c(qos) }
    };
    if let Ok(mut g) = tt.qos.lock() {
        *g = new_qos;
    }
    ZeroDdsStatus::Ok as c_int
}

/// InconsistentTopicStatus (Spec §2.2.4.2.4).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsInconsistentTopicStatus {
    /// Total count of inconsistent topic detections seen so far.
    pub total_count: i32,
    /// Change in total_count since last get-call (Spec read-and-clear).
    pub total_count_change: i32,
}

/// Liefert den InconsistentTopicStatus.
///
/// # Safety
/// `t` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_inconsistent_topic_status(
    t: *mut ZeroDdsTopic,
    out: *mut ZeroDdsInconsistentTopicStatus,
) -> c_int {
    if t.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: NULL-Checks oben.
    unsafe {
        *out = ZeroDdsInconsistentTopicStatus::default();
    }
    ZeroDdsStatus::Ok as c_int
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::factory_ffi::{
        zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
    };
    use crate::participant_ffi::{
        zerodds_dp_create_topic, zerodds_dp_delete_contained_entities, zerodds_dp_delete_topic,
    };
    use std::ffi::CStr;

    fn mk_participant(domain: u32) -> *mut ZeroDdsDomainParticipant {
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dpf_create_participant(f, domain, ptr::null()) }
    }

    #[test]
    fn topic_get_name_and_type_roundtrip() {
        let p = mk_participant(31);
        let n = c"Hello";
        let tn = c"WorldType";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let t = unsafe { zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null()) };
        assert!(!t.is_null());
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let raw_n = unsafe { zerodds_topic_get_name(t) };
        assert!(!raw_n.is_null());
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let s_n = unsafe { CStr::from_ptr(raw_n) }.to_str().unwrap();
        assert_eq!(s_n, "Hello");
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_string_free(raw_n) };

        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let raw_tn = unsafe { zerodds_topic_get_type_name(t) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let s_tn = unsafe { CStr::from_ptr(raw_tn) }.to_str().unwrap();
        assert_eq!(s_tn, "WorldType");
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_string_free(raw_tn) };

        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert_eq!(unsafe { zerodds_topic_get_participant(t) }, p);

        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dp_delete_topic(p, t) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dp_delete_contained_entities(p) };
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dpf_delete_participant(f, p) };
    }

    #[test]
    fn inconsistent_topic_status_default() {
        let p = mk_participant(32);
        let n = c"T";
        let tn = c"TT";
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let t = unsafe { zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null()) };
        let mut s = ZeroDdsInconsistentTopicStatus::default();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_topic_get_inconsistent_topic_status(t, &mut s) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(s.total_count, 0);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dp_delete_topic(p, t) };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dp_delete_contained_entities(p) };
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let _ = unsafe { zerodds_dpf_delete_participant(f, p) };
    }
}
