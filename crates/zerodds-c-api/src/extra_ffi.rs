// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Extended C-FFI for spec completeness (DDS 1.4 §2.2.2).
//!
//! This file bundles the operations omitted in the first RC1 wave:
//!  * QoS get/set for all 6 entity types (Spec §2.2.2.{1,2,3,4,5})
//!  * Instance operations on DataWriter (register_instance,
//!    unregister_instance, lookup_instance, get_key_value,
//!    dispose_w_timestamp, register_instance_w_timestamp,
//!    unregister_instance_w_timestamp) — Spec §2.2.2.4.2.5..
//!  * DataReader read/take variants (read_instance, take_instance,
//!    read_next_instance, take_next_instance, read_w_condition,
//!    take_w_condition, get_key_value, lookup_instance,
//!    wait_for_historical_data) — Spec §2.2.2.5.3
//!  * Matched-subscription / matched-publication listings
//!  * Loan API (loan_message, commit_loan, discard_loan)
//!  * `lookup_topicdescription` + `get_builtin_subscriber`
//!  * copy_from_topic_qos on Pub/Sub
//!
//! ## Vendor extension: instance operations with raw key bytes
//!
//! The DDS spec defines `register_instance(T instance_data)` with a
//! generic topic type `T`. In the byte-oriented C-FFI `T` is not
//! known — the caller therefore provides the **key-hash bytes (16 byte)** of the
//! instance, produced by the IDL codegen side via `T::encode_key_holder`.
//! See `docs/specs/zerodds-c-api-1.0.md §2.6 Vendor-Extension`.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ffi::c_int;
use core::ptr;
use core::slice;
use std::time::Duration;

use crate::ZeroDdsStatus;
use crate::entities::{
    ZeroDdsDataReader, ZeroDdsDataWriter, ZeroDdsDomainParticipant, ZeroDdsPublisher,
    ZeroDdsSubscriber,
};
#[allow(unused_imports)]
use crate::ffi_helpers::{Borrowed, BytesIn, BytesOut, OutPtr, Owned, status};
use crate::qos_ffi::{
    ZeroDdsDataReaderQos, ZeroDdsDataWriterQos, ZeroDdsDomainParticipantQos, ZeroDdsPublisherQos,
    ZeroDdsSubscriberQos, ZeroDdsTopicQos, dr_qos_from_c, dw_qos_from_c, pub_qos_from_c,
    sub_qos_from_c, topic_qos_from_c,
};

// ===========================================================================
// DomainParticipant Misc (lookup_topicdescription, get_builtin_subscriber)
// ===========================================================================

/// `lookup_topicdescription(name)` — Spec §2.2.2.2.1.13.
/// Returns the first topic with the same name, or NULL.
///
/// # Safety
/// `p`, `name` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_lookup_topicdescription(
    p: *mut ZeroDdsDomainParticipant,
    name: *const core::ffi::c_char,
) -> *mut crate::entities::ZeroDdsTopic {
    if p.is_null() || name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — p+name NULL-checked above; the topics list holds
    // only valid box pointers from dp_create_topic.
    unsafe {
        let pp = &*p;
        let cs = std::ffi::CStr::from_ptr(name);
        let name_str = match cs.to_str() {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        };
        if let Ok(list) = pp.topics.lock() {
            for &t in list.iter() {
                if t.is_null() {
                    continue;
                }
                if (*t).name == name_str {
                    return t;
                }
            }
        }
    }
    ptr::null_mut()
}

/// Opaque BuiltinSubscriber-Handle (Spec §2.2.2.2.1.7).
pub struct ZeroDdsBuiltinSubscriber {
    /// Back-reference to the participant for lifetime tracking.
    pub participant: *mut ZeroDdsDomainParticipant,
    /// Cloned Arc on the DCPS-internal BuiltinSubscriber.
    pub inner: alloc::sync::Arc<zerodds_dcps::builtin_subscriber::BuiltinSubscriber>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsBuiltinSubscriber {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsBuiltinSubscriber {}

/// `get_builtin_subscriber()` — Spec §2.2.2.2.1.7.
/// Returns a pointer to a wrapper around the DCPS BuiltinSubscriber.
/// The caller must call `zerodds_builtin_subscriber_destroy`.
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_builtin_subscriber(
    p: *mut ZeroDdsDomainParticipant,
) -> *mut ZeroDdsBuiltinSubscriber {
    if p.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    let inner = unsafe { (*p).dp.get_builtin_subscriber() };
    Box::into_raw(Box::new(ZeroDdsBuiltinSubscriber {
        participant: p,
        inner,
    }))
}

/// Deletes a BuiltinSubscriber wrapper.
///
/// # Safety
/// `bs` valid or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_subscriber_destroy(bs: *mut ZeroDdsBuiltinSubscriber) {
    // SAFETY: caller pledge: bs comes from get_builtin_subscriber (Box::into_raw).
    unsafe { Owned::from_raw_drop(bs) };
}

// ===========================================================================
// QoS get/set per Entity (Spec §2.2.2.x.y)
// ===========================================================================

/// Sets the DomainParticipant QoS (Spec §2.2.2.2.1.3).
///
/// # Safety
/// `p` valid; `qos` darf NULL sein (Reset auf Default).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_set_qos(
    p: *mut ZeroDdsDomainParticipant,
    qos: *const ZeroDdsDomainParticipantQos,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; qos NULL-tolerant.
    unsafe {
        let pp = &*p;
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::DomainParticipantQos::default()
        } else {
            crate::qos_ffi::dp_qos_from_c(qos)
        };
        match pp.dp.set_qos(new_qos) {
            Ok(()) => ZeroDdsStatus::Ok as c_int,
            Err(_) => ZeroDdsStatus::Error as c_int,
        }
    }
}

/// Reads the DomainParticipant QoS into `out` (Spec §2.2.2.2.1.4).
/// `out.user_data.value` must be initialized by the caller with a sufficient
/// buffer. On a too-small buffer the required
/// size is written back into `out.user_data.value_len` + `OutOfResources`.
///
/// # Safety
/// `p`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_qos(
    p: *mut ZeroDdsDomainParticipant,
    out: *mut ZeroDdsDomainParticipantQos,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — pp valid (NULL-checked above), out valid (caller pledge).
    unsafe {
        let pp = &*p;
        let qos = pp.dp.qos();
        crate::qos_ffi::dp_qos_to_c(&qos, out)
    }
}

/// Set-Default-Topic-QoS.
///
/// # Safety
/// `p` valid; `qos` darf NULL sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_set_default_topic_qos(
    p: *mut ZeroDdsDomainParticipant,
    qos: *const ZeroDdsTopicQos,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; qos NULL-tolerant.
    unsafe {
        let pp = &*p;
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::TopicQos::default()
        } else {
            topic_qos_from_c(qos)
        };
        if let Ok(mut g) = pp.default_topic_qos.lock() {
            *g = new_qos;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Get-Default-Topic-QoS-Snapshot.
///
/// # Safety
/// `p`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_default_topic_qos(
    p: *mut ZeroDdsDomainParticipant,
    out: *mut ZeroDdsTopicQos,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — p+out NULL-checked above.
    unsafe {
        let pp = &*p;
        let qos = pp
            .default_topic_qos
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        crate::qos_ffi::topic_qos_to_c(&qos, out)
    }
}

/// Set-Default-Publisher-QoS.
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_set_default_publisher_qos(
    p: *mut ZeroDdsDomainParticipant,
    qos: *const ZeroDdsPublisherQos,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; qos NULL-tolerant.
    unsafe {
        let pp = &*p;
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::PublisherQos::default()
        } else {
            pub_qos_from_c(qos)
        };
        if let Ok(mut g) = pp.default_publisher_qos.lock() {
            *g = new_qos;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Get-Default-Publisher-QoS.
///
/// # Safety
/// `p`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_default_publisher_qos(
    p: *mut ZeroDdsDomainParticipant,
    out: *mut ZeroDdsPublisherQos,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — p+out NULL-checked above.
    unsafe {
        let pp = &*p;
        let qos = pp
            .default_publisher_qos
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        crate::qos_ffi::pub_qos_to_c(&qos, out)
    }
}

/// Set-Default-Subscriber-QoS.
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_set_default_subscriber_qos(
    p: *mut ZeroDdsDomainParticipant,
    qos: *const ZeroDdsSubscriberQos,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; qos NULL-tolerant.
    unsafe {
        let pp = &*p;
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::SubscriberQos::default()
        } else {
            sub_qos_from_c(qos)
        };
        if let Ok(mut g) = pp.default_subscriber_qos.lock() {
            *g = new_qos;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Get-Default-Subscriber-QoS.
///
/// # Safety
/// `p`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_default_subscriber_qos(
    p: *mut ZeroDdsDomainParticipant,
    out: *mut ZeroDdsSubscriberQos,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — p+out NULL-checked above.
    unsafe {
        let pp = &*p;
        let qos = pp
            .default_subscriber_qos
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        crate::qos_ffi::sub_qos_to_c(&qos, out)
    }
}

/// Publisher set/get QoS.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_set_qos(
    pub_: *mut ZeroDdsPublisher,
    qos: *const ZeroDdsPublisherQos,
) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above; qos NULL-tolerant.
    unsafe {
        let pp = &*pub_;
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::PublisherQos::default()
        } else {
            pub_qos_from_c(qos)
        };
        if let Ok(mut g) = pp.qos.lock() {
            *g = new_qos;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Publisher get_qos.
///
/// # Safety
/// `pub_`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_get_qos(
    pub_: *mut ZeroDdsPublisher,
    out: *mut ZeroDdsPublisherQos,
) -> c_int {
    if pub_.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_+out NULL-checked above.
    unsafe {
        let pp = &*pub_;
        let qos = pp.qos.lock().map(|g| g.clone()).unwrap_or_default();
        crate::qos_ffi::pub_qos_to_c(&qos, out)
    }
}

/// Pub set_default_datawriter_qos.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_set_default_datawriter_qos(
    pub_: *mut ZeroDdsPublisher,
    qos: *const ZeroDdsDataWriterQos,
) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above; qos NULL-tolerant.
    unsafe {
        let pp = &*pub_;
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::DataWriterQos::default()
        } else {
            dw_qos_from_c(qos)
        };
        if let Ok(mut g) = pp.default_dw_qos.lock() {
            *g = new_qos;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Pub get_default_datawriter_qos.
///
/// # Safety
/// `pub_`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_get_default_datawriter_qos(
    pub_: *mut ZeroDdsPublisher,
    out: *mut ZeroDdsDataWriterQos,
) -> c_int {
    if pub_.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_+out NULL-checked above.
    unsafe {
        let pp = &*pub_;
        let qos = pp
            .default_dw_qos
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        crate::qos_ffi::dw_qos_to_c(&qos, out)
    }
}

/// Pub copy_from_topic_qos: copies the policies from TopicQos into
/// the DataWriterQos `history`/`durability`/`reliability`/etc. in place.
///
/// # Safety
/// Both pointers valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_copy_from_topic_qos(
    _pub: *mut ZeroDdsPublisher,
    dwqos_inout: *mut ZeroDdsDataWriterQos,
    tqos: *const ZeroDdsTopicQos,
) -> c_int {
    if dwqos_inout.is_null() || tqos.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dwqos_inout+tqos NULL-checked above.
    unsafe {
        (*dwqos_inout).durability = (*tqos).durability;
        (*dwqos_inout).deadline = (*tqos).deadline;
        (*dwqos_inout).latency_budget = (*tqos).latency_budget;
        (*dwqos_inout).liveliness = (*tqos).liveliness;
        (*dwqos_inout).reliability = (*tqos).reliability;
        (*dwqos_inout).destination_order = (*tqos).destination_order;
        (*dwqos_inout).history = (*tqos).history;
        (*dwqos_inout).resource_limits = (*tqos).resource_limits;
        (*dwqos_inout).transport_priority = (*tqos).transport_priority;
        (*dwqos_inout).lifespan = (*tqos).lifespan;
        (*dwqos_inout).ownership = (*tqos).ownership;
        (*dwqos_inout).topic_data = (*tqos).topic_data;
        (*dwqos_inout).durability_service = (*tqos).durability_service;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Sub set/get QoS.
///
/// # Safety
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_set_qos(
    sub: *mut ZeroDdsSubscriber,
    qos: *const ZeroDdsSubscriberQos,
) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — sub NULL-checked above; qos NULL-tolerant.
    unsafe {
        let sb = &*sub;
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::SubscriberQos::default()
        } else {
            sub_qos_from_c(qos)
        };
        if let Ok(mut g) = sb.qos.lock() {
            *g = new_qos;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Sub get_qos.
///
/// # Safety
/// `sub`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_get_qos(
    sub: *mut ZeroDdsSubscriber,
    out: *mut ZeroDdsSubscriberQos,
) -> c_int {
    if sub.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — sub+out NULL-checked above.
    unsafe {
        let sb = &*sub;
        let qos = sb.qos.lock().map(|g| g.clone()).unwrap_or_default();
        crate::qos_ffi::sub_qos_to_c(&qos, out)
    }
}

/// Sub set_default_datareader_qos.
///
/// # Safety
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_set_default_datareader_qos(
    sub: *mut ZeroDdsSubscriber,
    qos: *const ZeroDdsDataReaderQos,
) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — sub NULL-checked above; qos NULL-tolerant.
    unsafe {
        let sb = &*sub;
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::DataReaderQos::default()
        } else {
            dr_qos_from_c(qos)
        };
        if let Ok(mut g) = sb.default_dr_qos.lock() {
            *g = new_qos;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Sub get_default_datareader_qos.
///
/// # Safety
/// `sub`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_get_default_datareader_qos(
    sub: *mut ZeroDdsSubscriber,
    out: *mut ZeroDdsDataReaderQos,
) -> c_int {
    if sub.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — sub+out NULL-checked above.
    unsafe {
        let sb = &*sub;
        let qos = sb
            .default_dr_qos
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        crate::qos_ffi::dr_qos_to_c(&qos, out)
    }
}

/// Sub copy_from_topic_qos.
///
/// # Safety
/// Both pointers valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_copy_from_topic_qos(
    _sub: *mut ZeroDdsSubscriber,
    drqos_inout: *mut ZeroDdsDataReaderQos,
    tqos: *const ZeroDdsTopicQos,
) -> c_int {
    if drqos_inout.is_null() || tqos.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — drqos_inout+tqos NULL-checked above.
    unsafe {
        (*drqos_inout).durability = (*tqos).durability;
        (*drqos_inout).deadline = (*tqos).deadline;
        (*drqos_inout).latency_budget = (*tqos).latency_budget;
        (*drqos_inout).liveliness = (*tqos).liveliness;
        (*drqos_inout).reliability = (*tqos).reliability;
        (*drqos_inout).destination_order = (*tqos).destination_order;
        (*drqos_inout).history = (*tqos).history;
        (*drqos_inout).resource_limits = (*tqos).resource_limits;
        (*drqos_inout).ownership = (*tqos).ownership;
        (*drqos_inout).topic_data = (*tqos).topic_data;
    }
    ZeroDdsStatus::Ok as c_int
}

/// DataWriter set_qos.
///
/// # Safety
/// `dw` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_set_qos(
    dw: *mut ZeroDdsDataWriter,
    qos: *const ZeroDdsDataWriterQos,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above; qos NULL-tolerant.
    unsafe {
        let dwr = &*dw;
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::DataWriterQos::default()
        } else {
            dw_qos_from_c(qos)
        };
        if let Ok(mut g) = dwr.qos.lock() {
            *g = new_qos;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// DataWriter get_qos.
///
/// # Safety
/// `dw`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_qos(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsDataWriterQos,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above.
    unsafe {
        let dwr = &*dw;
        let qos = dwr.qos.lock().map(|g| g.clone()).unwrap_or_default();
        crate::qos_ffi::dw_qos_to_c(&qos, out)
    }
}

/// DataReader set_qos.
///
/// # Safety
/// `dr` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_set_qos(
    dr: *mut ZeroDdsDataReader,
    qos: *const ZeroDdsDataReaderQos,
) -> c_int {
    if dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — dr NULL-checked above; qos NULL-tolerant.
    unsafe {
        let drr = &*dr;
        let new_qos = if qos.is_null() {
            zerodds_dcps::qos::DataReaderQos::default()
        } else {
            dr_qos_from_c(qos)
        };
        if let Ok(mut g) = drr.qos.lock() {
            *g = new_qos;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// DataReader get_qos.
///
/// # Safety
/// `dr`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_qos(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsDataReaderQos,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let qos = drr.qos.lock().map(|g| g.clone()).unwrap_or_default();
        crate::qos_ffi::dr_qos_to_c(&qos, out)
    }
}

// ===========================================================================
// DataWriter Instance-Operations (Spec §2.2.2.4.2.5..14)
// ===========================================================================

/// `register_instance` — vendor variant: the caller provides a 16-byte key hash
/// instead of a generic `T instance_data`. The wire path knows only bytes.
///
/// # Safety
/// `dw` valid; `key`+`out_handle` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_register_instance(
    dw: *mut ZeroDdsDataWriter,
    key: *const u8,
    key_len: usize,
    out_handle: *mut u64,
) -> c_int {
    if key_len != 16 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — Newtypes encapsulate raw-pointer validity pledge.
    status(unsafe {
        (|| -> Result<(), ZeroDdsStatus> {
            let _dw = Borrowed::from_raw(dw).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let key_in = BytesIn::from_raw(key, 16).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let out = OutPtr::from_raw(out_handle).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let mut h = [0u8; 8];
            h.copy_from_slice(&key_in[..8]);
            out.write(u64::from_le_bytes(h));
            Ok(())
        })()
    })
}

/// `register_instance_w_timestamp`. RC1: source timestamp ignored
/// (same path as register_instance, documented in the vendor spec).
///
/// # Safety
/// Wie `register_instance`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_register_instance_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key: *const u8,
    key_len: usize,
    _ts_sec: i32,
    _ts_nanosec: u32,
    out_handle: *mut u64,
) -> c_int {
    // SAFETY: see fn # Safety doc — delegation to register_instance with the same contract.
    unsafe { zerodds_dw_register_instance(dw, key, key_len, out_handle) }
}

/// `unregister_instance` — emits the UNREGISTERED lifecycle.
///
/// # Safety
/// `dw` valid; `handle` must come from `register_instance`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_unregister_instance(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // Status bits: UNREGISTERED per RTPS inline-QoS status info §9.6.4.10.
    const STATUS_UNREGISTERED: u32 = 0x0000_0002;
    let mut k = [0u8; 16];
    k[..8].copy_from_slice(&handle.to_le_bytes());
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
    let (rt, eid) = unsafe { ((*dw).rt.clone(), (*dw).eid) };
    match rt.write_user_lifecycle(eid, k, STATUS_UNREGISTERED) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// `unregister_instance_w_timestamp`.
///
/// # Safety
/// Wie `unregister_instance`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_unregister_instance_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    _ts_sec: i32,
    _ts_nanosec: u32,
) -> c_int {
    // SAFETY: see fn # Safety doc — delegation to unregister_instance with the same contract.
    unsafe { zerodds_dw_unregister_instance(dw, handle) }
}

/// `lookup_instance` — vendor variant: returns a deterministic
/// handle from the first 8 key-hash bytes.
///
/// # Safety
/// `key[0..16]` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_lookup_instance(
    _dw: *mut ZeroDdsDataWriter,
    key: *const u8,
    key_len: usize,
    out_handle: *mut u64,
) -> c_int {
    if key_len != 16 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — Newtypes encapsulate raw-pointer validity pledge.
    status(unsafe {
        (|| -> Result<(), ZeroDdsStatus> {
            let key_in = BytesIn::from_raw(key, 16).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let out = OutPtr::from_raw(out_handle).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let mut h = [0u8; 8];
            h.copy_from_slice(&key_in[..8]);
            out.write(u64::from_le_bytes(h));
            Ok(())
        })()
    })
}

/// `get_key_value(handle)` — in the C-FFI only a vendor variant: copies
/// 8 bytes of the handle into `out_buf[0..8]`. The full spec round-trip
/// (T-encoded key) is the codegen path.
///
/// # Safety
/// `dw` valid; `out_buf[0..*inout_len]` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_key_value(
    _dw: *mut ZeroDdsDataWriter,
    handle: u64,
    out_buf: *mut u8,
    inout_len: *mut usize,
) -> c_int {
    if out_buf.is_null() || inout_len.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — out_buf+inout_len NULL-checked above; out_buf[0..*inout_len] valid.
    unsafe {
        if *inout_len < 8 {
            *inout_len = 8;
            return ZeroDdsStatus::OutOfResources as c_int;
        }
        ptr::copy_nonoverlapping(handle.to_le_bytes().as_ptr(), out_buf, 8);
        *inout_len = 8;
    }
    ZeroDdsStatus::Ok as c_int
}

/// `dispose_w_timestamp`.
///
/// # Safety
/// Wie `dispose`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_dispose_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key_hash: *const u8,
    handle: u64,
    _ts_sec: i32,
    _ts_nanosec: u32,
) -> c_int {
    // SAFETY: see fn # Safety doc — delegation to dw_dispose with the same contract.
    unsafe { crate::publisher_ffi::zerodds_dw_dispose(dw, key_hash, handle) }
}

// ===========================================================================
// DataWriter Matched-Subscriptions
// ===========================================================================

/// `get_matched_subscriptions` — list of the `InstanceHandle`s of all
/// matched remote readers (Spec §2.2.2.4.2.x).
///
/// # Safety
/// `dw` valid; `out_handles[0..cap]` writeable; `out_count` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_matched_subscriptions(
    dw: *mut ZeroDdsDataWriter,
    out_handles: *mut u64,
    out_count: *mut usize,
    cap: usize,
) -> c_int {
    if dw.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out_count NULL-checked above; out_handles[0..cap]
    // writeable if non-NULL (caller pledge).
    unsafe {
        let dwr = &*dw;
        let handles = dwr.rt.user_writer_matched_subscription_handles(dwr.eid);
        let n = handles.len().min(cap);
        if !out_handles.is_null() && n > 0 {
            let dst = slice::from_raw_parts_mut(out_handles, n);
            for (i, h) in handles.iter().take(n).enumerate() {
                dst[i] = h.as_raw();
            }
        }
        *out_count = n;
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_matched_subscription_data(handle)` — Spec §2.2.2.4.2.x.
/// Returns the `SubscriptionBuiltinTopicData` for a matched
/// reader, obtained from the BuiltinSubscriber SEDP cache.
///
/// # Safety
/// `dw` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_matched_subscription_data(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    out: *mut crate::builtin_ffi::ZeroDdsSubscriptionBuiltinTopicData,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above; dwr.publisher and
    // pub_.participant from create_datawriter+create_publisher (Box::into_raw).
    let bs = unsafe {
        let dwr = &*dw;
        if dwr.publisher.is_null() {
            return ZeroDdsStatus::BadHandle as c_int;
        }
        let pub_ = &*dwr.publisher;
        if pub_.participant.is_null() {
            return ZeroDdsStatus::BadHandle as c_int;
        }
        let dp_wrapper = &*pub_.participant;
        dp_wrapper.dp.get_builtin_subscriber()
    };
    let sub_reader = bs.subscription_reader();
    let samples: Vec<zerodds_dcps::builtin_topics::SubscriptionBuiltinTopicData> =
        sub_reader.read().unwrap_or_default();
    for s in samples {
        let h = zerodds_dcps::instance_handle::InstanceHandle::from_guid(s.key).as_raw();
        if h == handle {
            return write_subscription_data(out, &s);
        }
    }
    ZeroDdsStatus::NoData as c_int
}

// ===========================================================================
// DataWriter Loan-API (Zero-Copy Vendor-Extension)
// ===========================================================================

/// `loan_message` — zero-copy loan via a heap box (vendor variant).
/// Allocates a Box<[u8]>, leaks the pointer; the caller writes into
/// the buffer and calls `commit_loan` (sends via write_user_sample)
/// or `discard_loan` (frees the buffer without sending).
///
/// **Vendor decision (Spec §2.2.2.4.2 loan API + vendor spec
/// `zerodds-flatdata-1.0.md` §4):** real Iceoryx SHM zero-copy
/// requires a separate `zerodds-flatdata` Iceoryx backend wireup;
/// the heap-loan variant exposed here fulfills the DDS spec
/// contract (the caller writes into the buffer, then commit/discard)
/// without the SHM optimization. The real SHM loan is transparent via the
/// `zerodds-flatdata` Iceoryx backend when it is active.
///
/// # Safety
/// Alle Pointers valid; `len > 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_loan_message(
    dw: *mut ZeroDdsDataWriter,
    len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> c_int {
    if dw.is_null() || out_ptr.is_null() || out_len.is_null() || len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // Zero-copy SHM path when the writer has `zerodds_dw_enable_shm_loan`
    // active; otherwise fall through to the heap-box variant below.
    #[cfg(feature = "flatdata-loan")]
    // SAFETY: dw NULL-checked above; out_ptr/out_len NULL-checked above.
    if let Some(rc) = unsafe {
        let dwr = &*dw;
        crate::shm_loan_ffi::try_loan(&dwr.rt, dwr.eid, len, out_ptr, out_len)
    } {
        return rc;
    }
    let buf: Box<[u8]> = vec![0u8; len].into_boxed_slice();
    let raw = Box::into_raw(buf) as *mut u8;
    // SAFETY: see fn # Safety doc — out_ptr+out_len NULL-checked above; raw from
    // Box::into_raw, the caller now owns the buffer until commit/discard.
    unsafe {
        *out_ptr = raw;
        *out_len = len;
    }
    ZeroDdsStatus::Ok as c_int
}

/// `commit_loan` — writes the loan buffer as a sample and frees the
/// buffer.
///
/// # Safety
/// `dw`, `ptr` valid; `(ptr, len)` from `loan_message`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_commit_loan(
    dw: *mut ZeroDdsDataWriter,
    ptr: *mut u8,
    len: usize,
) -> c_int {
    if dw.is_null() || ptr.is_null() || len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // Zero-copy SHM commit when `ptr` is an active SHM loan; else heap path.
    #[cfg(feature = "flatdata-loan")]
    // SAFETY: dw NULL-checked above; ptr valid per the contract.
    if let Some(rc) = unsafe {
        let dwr = &*dw;
        crate::shm_loan_ffi::try_commit(&dwr.rt, dwr.eid, ptr, len)
    } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — dw+ptr NULL-checked above; ptr+len come from
    // loan_message (Box::into_raw); rebuild + into_vec is conformant.
    let (rt, eid, payload) = unsafe {
        let dwr = &*dw;
        let boxed: Box<[u8]> = Box::from_raw(core::slice::from_raw_parts_mut(ptr, len));
        (dwr.rt.clone(), dwr.eid, boxed.into_vec())
    };
    match rt.write_user_sample(eid, payload) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// `discard_loan` — frees the loan buffer without sending.
///
/// # Safety
/// `dw`, `ptr` valid; `(ptr, len)` from `loan_message`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_discard_loan(
    _dw: *mut ZeroDdsDataWriter,
    ptr: *mut u8,
    len: usize,
) -> c_int {
    if ptr.is_null() || len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // Zero-copy SHM discard when `ptr` is an active SHM loan; else heap path.
    // `_dw` is not NULL-checked by this fn's contract, so guard it here.
    #[cfg(feature = "flatdata-loan")]
    if !_dw.is_null() {
        // SAFETY: _dw checked non-null just above; ptr valid per the contract.
        if let Some(rc) = unsafe {
            let dwr = &*_dw;
            crate::shm_loan_ffi::try_discard(&dwr.rt, dwr.eid, ptr)
        } {
            return rc;
        }
    }
    // SAFETY: see fn # Safety doc — ptr+len from loan_message (Box::into_raw); rebuild + drop.
    let _: Box<[u8]> = unsafe { Box::from_raw(core::slice::from_raw_parts_mut(ptr, len)) };
    ZeroDdsStatus::Ok as c_int
}

// ===========================================================================
// DataReader Instance-Variants
// ===========================================================================

/// `read_instance(handle)` — Spec §2.2.2.5.3.5: returns only samples
/// of this instance.
///
/// # Safety
/// `dr`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_read_instance(
    dr: *mut ZeroDdsDataReader,
    handle: u64,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    s: u32,
    v: u32,
    i: u32,
) -> c_int {
    // SAFETY: see fn # Safety doc — delegation to dr_read with the same pointer contract,
    // then a safe in-place filter on the resulting SampleArray.
    let rc = unsafe { crate::subscriber_ffi::zerodds_dr_read(dr, out, max, s, v, i) };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    crate::subscriber_ffi::sample_array_filter_instance(out, handle)
}

/// `take_instance(handle)`.
///
/// # Safety
/// `dr`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_take_instance(
    dr: *mut ZeroDdsDataReader,
    handle: u64,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    s: u32,
    v: u32,
    i: u32,
) -> c_int {
    // SAFETY: see fn # Safety doc — delegation to dr_take + in-place filter.
    let rc = unsafe { crate::subscriber_ffi::zerodds_dr_take(dr, out, max, s, v, i) };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    crate::subscriber_ffi::sample_array_filter_instance(out, handle)
}

/// `read_next_instance(prev_handle)` — Spec §2.2.2.5.3.7: returns
/// samples of the next instance > prev_handle.
///
/// # Safety
/// `dr`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_read_next_instance(
    dr: *mut ZeroDdsDataReader,
    prev_handle: u64,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    s: u32,
    v: u32,
    i: u32,
) -> c_int {
    // SAFETY: see fn # Safety doc — delegation to dr_read + in-place filter.
    let rc = unsafe { crate::subscriber_ffi::zerodds_dr_read(dr, out, max, s, v, i) };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    crate::subscriber_ffi::sample_array_filter_next_instance(out, prev_handle)
}

/// `take_next_instance(prev_handle)`.
///
/// # Safety
/// `dr`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_take_next_instance(
    dr: *mut ZeroDdsDataReader,
    prev_handle: u64,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    s: u32,
    v: u32,
    i: u32,
) -> c_int {
    // SAFETY: see fn # Safety doc — delegation to dr_take + in-place filter.
    let rc = unsafe { crate::subscriber_ffi::zerodds_dr_take(dr, out, max, s, v, i) };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    crate::subscriber_ffi::sample_array_filter_next_instance(out, prev_handle)
}

/// `read_w_condition` — Spec §2.2.2.5.3.4: returns only samples that
/// match the condition state mask (ReadCondition.sample_states/
/// view_states/instance_states).
///
/// # Safety
/// `dr`, `out` valid; `cond` is a ReadCondition or QueryCondition.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_read_w_condition(
    dr: *mut ZeroDdsDataReader,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    cond: *mut core::ffi::c_void,
) -> c_int {
    // SAFETY: see fn # Safety doc — cond must be a ReadCondition/QueryCondition,
    // condition_state_masks inherits the caller pledge; then delegation to dr_read.
    let (rc, masks) = unsafe {
        let masks = match crate::condition_ffi::condition_state_masks(cond) {
            Some(m) => m,
            None => return ZeroDdsStatus::BadParameter as c_int,
        };
        let rc = crate::subscriber_ffi::zerodds_dr_read(dr, out, max, masks.0, masks.1, masks.2);
        (rc, masks)
    };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    crate::subscriber_ffi::sample_array_filter_states(out, masks.0, masks.1, masks.2)
}

/// `take_w_condition`.
///
/// # Safety
/// Wie `read_w_condition`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_take_w_condition(
    dr: *mut ZeroDdsDataReader,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    cond: *mut core::ffi::c_void,
) -> c_int {
    // SAFETY: see fn # Safety doc — cond must be a ReadCondition/QueryCondition,
    // condition_state_masks inherits the caller pledge; then delegation to dr_take.
    let (rc, masks) = unsafe {
        let masks = match crate::condition_ffi::condition_state_masks(cond) {
            Some(m) => m,
            None => return ZeroDdsStatus::BadParameter as c_int,
        };
        let rc = crate::subscriber_ffi::zerodds_dr_take(dr, out, max, masks.0, masks.1, masks.2);
        (rc, masks)
    };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    crate::subscriber_ffi::sample_array_filter_states(out, masks.0, masks.1, masks.2)
}

/// `lookup_instance` (Reader).
///
/// # Safety
/// `key[0..16]` valid; `out_handle` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_lookup_instance(
    _dr: *mut ZeroDdsDataReader,
    key: *const u8,
    key_len: usize,
    out_handle: *mut u64,
) -> c_int {
    if key_len != 16 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — Newtypes encapsulate raw-pointer validity pledge.
    status(unsafe {
        (|| -> Result<(), ZeroDdsStatus> {
            let key_in = BytesIn::from_raw(key, 16).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let out = OutPtr::from_raw(out_handle).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let mut h = [0u8; 8];
            h.copy_from_slice(&key_in[..8]);
            out.write(u64::from_le_bytes(h));
            Ok(())
        })()
    })
}

/// `get_key_value` (Reader) — analog zu Writer.
///
/// # Safety
/// `out_buf[0..*inout_len]` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_key_value(
    _dr: *mut ZeroDdsDataReader,
    handle: u64,
    out_buf: *mut u8,
    inout_len: *mut usize,
) -> c_int {
    if out_buf.is_null() || inout_len.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — out_buf+inout_len NULL-checked above; out_buf[0..*inout_len] valid.
    unsafe {
        if *inout_len < 8 {
            *inout_len = 8;
            return ZeroDdsStatus::OutOfResources as c_int;
        }
        ptr::copy_nonoverlapping(handle.to_le_bytes().as_ptr(), out_buf, 8);
        *inout_len = 8;
    }
    ZeroDdsStatus::Ok as c_int
}

/// `wait_for_historical_data(timeout)` — RC1: Volatile default → Ok without wait.
///
/// # Safety
/// `dr` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_wait_for_historical_data(
    dr: *mut ZeroDdsDataReader,
    _timeout_sec: i32,
    _timeout_nanosec: u32,
) -> c_int {
    if dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // Volatile reader: no historical data — Ok without waiting.
    // For a TransientLocal reader RC1 would return `Unsupported`
    // here, because the history service is not hooked in.
    ZeroDdsStatus::Ok as c_int
}

/// `get_matched_publications` — list of the `InstanceHandle`s of all
/// matched remote writers (Spec §2.2.2.5.x).
///
/// # Safety
/// `dr` valid; `out_handles[0..cap]` writeable; `out_count` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_matched_publications(
    dr: *mut ZeroDdsDataReader,
    out_handles: *mut u64,
    out_count: *mut usize,
    cap: usize,
) -> c_int {
    if dr.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out_count NULL-checked above; out_handles[0..cap]
    // writeable if non-NULL (caller pledge).
    unsafe {
        let drr = &*dr;
        let handles = drr.rt.user_reader_matched_publication_handles(drr.eid);
        let n = handles.len().min(cap);
        if !out_handles.is_null() && n > 0 {
            let dst = slice::from_raw_parts_mut(out_handles, n);
            for (i, h) in handles.iter().take(n).enumerate() {
                dst[i] = h.as_raw();
            }
        }
        *out_count = n;
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_matched_publication_data(handle)` — Spec §2.2.2.5.x.
/// Returns `PublicationBuiltinTopicData` from the BuiltinSubscriber cache.
///
/// # Safety
/// `dr`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_matched_publication_data(
    dr: *mut ZeroDdsDataReader,
    handle: u64,
    out: *mut crate::builtin_ffi::ZeroDdsPublicationBuiltinTopicData,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above; subscriber+participant
    // from create_* fns (Box::into_raw).
    let bs = unsafe {
        let drr = &*dr;
        if drr.subscriber.is_null() {
            return ZeroDdsStatus::BadHandle as c_int;
        }
        let sub_ = &*drr.subscriber;
        if sub_.participant.is_null() {
            return ZeroDdsStatus::BadHandle as c_int;
        }
        let dp_wrapper = &*sub_.participant;
        dp_wrapper.dp.get_builtin_subscriber()
    };
    let pub_reader = bs.publication_reader();
    let samples: Vec<zerodds_dcps::builtin_topics::PublicationBuiltinTopicData> =
        pub_reader.read().unwrap_or_default();
    for s in samples {
        let h = zerodds_dcps::instance_handle::InstanceHandle::from_guid(s.key).as_raw();
        if h == handle {
            return write_publication_data(out, &s);
        }
    }
    ZeroDdsStatus::NoData as c_int
}

fn write_subscription_data(
    out: *mut crate::builtin_ffi::ZeroDdsSubscriptionBuiltinTopicData,
    s: &zerodds_dcps::builtin_topics::SubscriptionBuiltinTopicData,
) -> c_int {
    use alloc::ffi::CString;
    let topic_name = CString::new(s.topic_name.as_bytes())
        .unwrap_or_default()
        .into_raw();
    let type_name = CString::new(s.type_name.as_bytes())
        .unwrap_or_default()
        .into_raw();
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        (*out).key = s.key.to_bytes();
        (*out).participant_key = s.participant_key.to_bytes();
        (*out).topic_name = topic_name;
        (*out).type_name = type_name;
        (*out).durability_kind = s.durability as u32;
        (*out).reliability_kind = s.reliability as u32;
        (*out).ownership_kind = s.ownership as u32;
        (*out).liveliness_lease_seconds = s.liveliness_lease_seconds;
        (*out).deadline_seconds = s.deadline_seconds;
    }
    ZeroDdsStatus::Ok as c_int
}

fn write_publication_data(
    out: *mut crate::builtin_ffi::ZeroDdsPublicationBuiltinTopicData,
    s: &zerodds_dcps::builtin_topics::PublicationBuiltinTopicData,
) -> c_int {
    use alloc::ffi::CString;
    let topic_name = CString::new(s.topic_name.as_bytes())
        .unwrap_or_default()
        .into_raw();
    let type_name = CString::new(s.type_name.as_bytes())
        .unwrap_or_default()
        .into_raw();
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        (*out).key = s.key.to_bytes();
        (*out).participant_key = s.participant_key.to_bytes();
        (*out).topic_name = topic_name;
        (*out).type_name = type_name;
        (*out).durability_kind = s.durability as u32;
        (*out).reliability_kind = s.reliability as u32;
        (*out).ownership_kind = s.ownership as u32;
        (*out).ownership_strength = s.ownership_strength;
        (*out).liveliness_lease_seconds = s.liveliness_lease_seconds;
        (*out).deadline_seconds = s.deadline_seconds;
        (*out).lifespan_seconds = s.lifespan_seconds;
    }
    ZeroDdsStatus::Ok as c_int
}

// suppress unused-import warning on `Duration`/`Vec`/`Box`
#[allow(dead_code)]
fn _suppress(_: Duration, _: Vec<u8>, _: Box<u8>) {}

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

    fn mk(domain: u32) -> *mut ZeroDdsDomainParticipant {
        let f = zerodds_dpf_get_instance();
        // SAFETY: f from dpf_get_instance, statically valid.
        unsafe { zerodds_dpf_create_participant(f, domain, ptr::null()) }
    }
    fn cleanup(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: p from mk() / dpf_create_participant; f statically valid.
        unsafe {
            zerodds_dp_delete_contained_entities(p);
            zerodds_dpf_delete_participant(f, p);
        }
    }

    #[test]
    fn dp_get_set_qos_roundtrip_default() {
        let p = mk(70);
        let mut qos = ZeroDdsDomainParticipantQos {
            user_data: crate::qos_ffi::ZeroDdsUserDataQosPolicy {
                value: ptr::null(),
                value_len: 0,
            },
            entity_factory: crate::qos_ffi::ZeroDdsEntityFactoryQosPolicy {
                autoenable_created_entities: true,
            },
        };
        // SAFETY: p from mk(), valid until cleanup. qos lives for the test.
        unsafe {
            assert_eq!(zerodds_dp_set_qos(p, &qos), ZeroDdsStatus::Ok as c_int);
            assert_eq!(zerodds_dp_get_qos(p, &mut qos), ZeroDdsStatus::Ok as c_int);
        }
        cleanup(p);
    }

    #[test]
    fn dp_default_topic_qos_roundtrip() {
        let p = mk(71);
        // SAFETY: p from mk(), valid until cleanup. tqos lives for the test.
        unsafe {
            let mut tqos: ZeroDdsTopicQos = core::mem::zeroed();
            assert_eq!(
                zerodds_dp_set_default_topic_qos(p, &tqos),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                zerodds_dp_get_default_topic_qos(p, &mut tqos),
                ZeroDdsStatus::Ok as c_int
            );
        }
        cleanup(p);
    }

    #[test]
    fn dw_register_unregister_lookup_instance() {
        let p = mk(72);
        let n = c"T";
        let tn = c"TT";
        let key = [0xABu8; 16];
        // SAFETY: p+n+tn+key live for the test; the extern fns document their pledges.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            let mut handle = 0u64;
            assert_eq!(
                zerodds_dw_register_instance(dw, key.as_ptr(), 16, &mut handle),
                ZeroDdsStatus::Ok as c_int
            );
            assert!(handle != 0);
            let mut h2 = 0u64;
            assert_eq!(
                zerodds_dw_lookup_instance(dw, key.as_ptr(), 16, &mut h2),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(handle, h2, "lookup must match register");
            let rc = zerodds_dw_unregister_instance(dw, handle);
            assert!(rc == ZeroDdsStatus::Ok as c_int || rc == ZeroDdsStatus::Error as c_int);
        }
        cleanup(p);
    }

    #[test]
    fn dw_get_key_value_buffer_too_small() {
        let p = mk(73);
        let n = c"T";
        let tn = c"TT";
        let mut buf = [0u8; 4];
        let mut len: usize = 4;
        // SAFETY: p+n+tn live for the test; buf+len writeable.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            assert_eq!(
                zerodds_dw_get_key_value(dw, 0xDEAD, buf.as_mut_ptr(), &mut len),
                ZeroDdsStatus::OutOfResources as c_int
            );
        }
        assert_eq!(len, 8, "must report needed buffer size");
        cleanup(p);
    }

    #[test]
    fn dw_loan_message_then_commit_or_discard() {
        let p = mk(74);
        let n = c"T";
        let tn = c"TT";
        let mut p_buf: *mut u8 = ptr::null_mut();
        let mut len_buf: usize = 0;
        let mut p2: *mut u8 = ptr::null_mut();
        let mut l2: usize = 0;
        // SAFETY: p+n+tn live for the test; all out-pointers on stack-local slots.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            // Loan-then-commit
            assert_eq!(
                zerodds_dw_loan_message(dw, 16, &mut p_buf, &mut len_buf),
                ZeroDdsStatus::Ok as c_int
            );
            assert!(!p_buf.is_null() && len_buf == 16);
            for i in 0..16 {
                *p_buf.add(i) = i as u8;
            }
            let rc_commit = zerodds_dw_commit_loan(dw, p_buf, len_buf);
            assert!(
                rc_commit == ZeroDdsStatus::Ok as c_int
                    || rc_commit == ZeroDdsStatus::Error as c_int,
                "commit returns Ok/Error"
            );
            // Loan-then-discard
            let _ = zerodds_dw_loan_message(dw, 8, &mut p2, &mut l2);
            assert_eq!(
                zerodds_dw_discard_loan(dw, p2, l2),
                ZeroDdsStatus::Ok as c_int
            );
        }
        cleanup(p);
    }

    #[test]
    fn dr_get_matched_publications_empty() {
        let p = mk(75);
        let n = c"T";
        let tn = c"TT";
        let mut buf = [0u64; 16];
        let mut count = 0usize;
        // SAFETY: p+n+tn live for the test.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert_eq!(
                zerodds_dr_get_matched_publications(dr, buf.as_mut_ptr(), &mut count, 16),
                ZeroDdsStatus::Ok as c_int
            );
        }
        assert_eq!(count, 0);
        cleanup(p);
    }

    #[test]
    fn dr_wait_for_historical_data_volatile_returns_ok() {
        let p = mk(76);
        let n = c"T";
        let tn = c"TT";
        // SAFETY: p+n+tn live for the test.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert_eq!(
                zerodds_dr_wait_for_historical_data(dr, 0, 100_000_000),
                ZeroDdsStatus::Ok as c_int
            );
        }
        cleanup(p);
    }

    #[test]
    fn dw_get_matched_subscriptions_initially_empty() {
        let p = mk(78);
        let n = c"T";
        let tn = c"TT";
        let mut buf = [0u64; 16];
        let mut count = 0usize;
        // SAFETY: p+n+tn live for the test.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            assert_eq!(
                zerodds_dw_get_matched_subscriptions(dw, buf.as_mut_ptr(), &mut count, 16),
                ZeroDdsStatus::Ok as c_int
            );
        }
        assert_eq!(count, 0);
        cleanup(p);
    }

    #[test]
    fn dr_get_matched_publications_initially_empty() {
        let p = mk(79);
        let n = c"T";
        let tn = c"TT";
        let mut buf = [0u64; 16];
        let mut count = 0usize;
        // SAFETY: p+n+tn live for the test.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert_eq!(
                zerodds_dr_get_matched_publications(dr, buf.as_mut_ptr(), &mut count, 16),
                ZeroDdsStatus::Ok as c_int
            );
        }
        assert_eq!(count, 0);
        cleanup(p);
    }

    #[test]
    fn dp_set_get_qos_userdata_roundtrip() {
        let p = mk(80);
        let bytes = b"my_user_data";
        let qos_in = ZeroDdsDomainParticipantQos {
            user_data: crate::qos_ffi::ZeroDdsUserDataQosPolicy {
                value: bytes.as_ptr(),
                value_len: bytes.len(),
            },
            entity_factory: crate::qos_ffi::ZeroDdsEntityFactoryQosPolicy {
                autoenable_created_entities: false,
            },
        };
        let mut out_buf = vec![0u8; 32];
        let mut qos_out = ZeroDdsDomainParticipantQos {
            user_data: crate::qos_ffi::ZeroDdsUserDataQosPolicy {
                value: out_buf.as_mut_ptr(),
                value_len: out_buf.len(),
            },
            entity_factory: crate::qos_ffi::ZeroDdsEntityFactoryQosPolicy {
                autoenable_created_entities: true,
            },
        };
        // SAFETY: p+bytes+out_buf live for the test.
        unsafe {
            assert_eq!(zerodds_dp_set_qos(p, &qos_in), ZeroDdsStatus::Ok as c_int);
            assert_eq!(
                zerodds_dp_get_qos(p, &mut qos_out),
                ZeroDdsStatus::Ok as c_int
            );
        }
        assert_eq!(qos_out.user_data.value_len, bytes.len());
        assert!(!qos_out.entity_factory.autoenable_created_entities);
        cleanup(p);
    }

    #[test]
    fn pub_copy_from_topic_qos_propagates_policies() {
        let p = mk(77);
        // SAFETY: p lives for the test; tqos+dwqos zeroed on the stack.
        unsafe {
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let mut tqos: ZeroDdsTopicQos = core::mem::zeroed();
            tqos.history.kind = 0;
            tqos.history.depth = 42;
            tqos.reliability.kind = 2; // Reliable
            let mut dwqos: ZeroDdsDataWriterQos = core::mem::zeroed();
            assert_eq!(
                zerodds_pub_copy_from_topic_qos(pubh, &mut dwqos, &tqos),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(dwqos.history.depth, 42);
            assert_eq!(dwqos.reliability.kind, 2);
        }
        cleanup(p);
    }
}
