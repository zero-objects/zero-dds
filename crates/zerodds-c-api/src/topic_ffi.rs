// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Topic C-FFI (Spec §2.2.2.3.1 + DDS-PSM-Cxx §7.5.4).

use alloc::ffi::CString;
use core::ffi::{c_char, c_int};
use core::ptr;

use crate::ZeroDdsStatus;
use crate::entities::{ZeroDdsDomainParticipant, ZeroDdsTopic};

/// Returns the topic name as a heap-allocated C string. The caller must
/// call `zerodds_string_free`.
///
/// # Safety
/// `t` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_name(t: *mut ZeroDdsTopic) -> *mut c_char {
    if t.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — t NULL-checked above.
    let name = unsafe { (*t).name.clone() };
    match CString::new(name) {
        Ok(c) => c.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Returns the topic type name. The caller must call `zerodds_string_free`.
///
/// # Safety
/// `t` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_type_name(t: *mut ZeroDdsTopic) -> *mut c_char {
    if t.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — t NULL-checked above.
    let type_name = unsafe { (*t).type_name.clone() };
    match CString::new(type_name) {
        Ok(c) => c.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Returns the owning participant.
///
/// # Safety
/// `t` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_participant(
    t: *mut ZeroDdsTopic,
) -> *mut ZeroDdsDomainParticipant {
    if t.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — t NULL-checked above.
    unsafe { (*t).participant }
}

/// Frees a C string previously allocated by `zerodds_topic_get_*`.
///
/// # Safety
/// `s` must come from a `zerodds_*_get_*` function that produces a
/// `*mut c_char` via `CString::into_raw`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc — s from CString::into_raw of a zerodds_*_get_* fn.
    let _ = unsafe { CString::from_raw(s) };
}

/// Get QoS into `out` (Spec §2.2.2.3.2.x).
/// `out` is `*mut ZeroDdsTopicQos` with a caller-supplied buffer for
/// `topic_data.value` (variable length).
///
/// # Safety
/// `t`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_qos(
    t: *mut ZeroDdsTopic,
    out: *mut crate::qos_ffi::ZeroDdsTopicQos,
) -> c_int {
    if t.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — t+out NULL-checked above; topic_qos_to_c
    // handles variable-length fields.
    unsafe {
        let qos = (*t).qos.lock().map(|g| g.clone()).unwrap_or_default();
        crate::qos_ffi::topic_qos_to_c(&qos, out)
    }
}

/// Set QoS (Spec §2.2.2.3.2.x). NULL = default.
///
/// # Safety
/// `t` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_set_qos(
    t: *mut ZeroDdsTopic,
    qos: *const crate::qos_ffi::ZeroDdsTopicQos,
) -> c_int {
    if t.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — t NULL-checked above; qos NULL-tolerant.
    unsafe {
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::TopicQos::default()
        } else {
            crate::qos_ffi::topic_qos_from_c(qos)
        };
        if let Ok(mut g) = (*t).qos.lock() {
            *g = new_qos;
        }
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

/// Returns the InconsistentTopicStatus.
///
/// # Safety
/// `t` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_inconsistent_topic_status(
    t: *mut ZeroDdsTopic,
    out: *mut ZeroDdsInconsistentTopicStatus,
) -> c_int {
    if t.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — t+out NULL-checked above.
    unsafe { *out = ZeroDdsInconsistentTopicStatus::default() };
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
        // SAFETY: f from dpf_get_instance, statically valid.
        unsafe { zerodds_dpf_create_participant(f, domain, ptr::null()) }
    }

    #[test]
    fn topic_get_name_and_type_roundtrip() {
        let p = mk_participant(31);
        let n = c"Hello";
        let tn = c"WorldType";
        let f = zerodds_dpf_get_instance();
        // SAFETY: p from mk; n+tn static; f static; raw_* C strings from zerodds_topic_get_*.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            assert!(!t.is_null());
            let raw_n = zerodds_topic_get_name(t);
            assert!(!raw_n.is_null());
            let s_n = CStr::from_ptr(raw_n).to_str().unwrap();
            assert_eq!(s_n, "Hello");
            zerodds_string_free(raw_n);

            let raw_tn = zerodds_topic_get_type_name(t);
            let s_tn = CStr::from_ptr(raw_tn).to_str().unwrap();
            assert_eq!(s_tn, "WorldType");
            zerodds_string_free(raw_tn);

            assert_eq!(zerodds_topic_get_participant(t), p);
            let _ = zerodds_dp_delete_topic(p, t);
            zerodds_dp_delete_contained_entities(p);
            let _ = zerodds_dpf_delete_participant(f, p);
        }
    }

    #[test]
    fn inconsistent_topic_status_default() {
        let p = mk_participant(32);
        let n = c"T";
        let tn = c"TT";
        let f = zerodds_dpf_get_instance();
        let mut s = ZeroDdsInconsistentTopicStatus::default();
        // SAFETY: p from mk; n+tn static; f static; s stack-local.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let rc = zerodds_topic_get_inconsistent_topic_status(t, &mut s);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(s.total_count, 0);
            let _ = zerodds_dp_delete_topic(p, t);
            zerodds_dp_delete_contained_entities(p);
            let _ = zerodds_dpf_delete_participant(f, p);
        }
    }
}
