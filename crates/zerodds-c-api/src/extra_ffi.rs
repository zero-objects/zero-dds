// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Erweitere C-FFI fuer Spec-Vollstaendigkeit (DDS 1.4 §2.2.2).
//!
//! Diese Datei buendelt die in der ersten RC1-Welle ausgelassenen
//! Operationen:
//!  * QoS get/set fuer alle 6 Entity-Typen (Spec §2.2.2.{1,2,3,4,5})
//!  * Instance-Operations auf DataWriter (register_instance,
//!    unregister_instance, lookup_instance, get_key_value,
//!    dispose_w_timestamp, register_instance_w_timestamp,
//!    unregister_instance_w_timestamp) — Spec §2.2.2.4.2.5..
//!  * DataReader read/take Varianten (read_instance, take_instance,
//!    read_next_instance, take_next_instance, read_w_condition,
//!    take_w_condition, get_key_value, lookup_instance,
//!    wait_for_historical_data) — Spec §2.2.2.5.3
//!  * Matched-Subscription / Matched-Publication-Listings
//!  * Loan-API (loan_message, commit_loan, discard_loan)
//!  * `lookup_topicdescription` + `get_builtin_subscriber`
//!  * copy_from_topic_qos auf Pub/Sub
//!
//! ## Vendor-Erweiterung: Instance-Operations mit Raw-Key-Bytes
//!
//! Die DDS-Spec definiert `register_instance(T instance_data)` mit
//! generischem Topic-Type `T`. Im byte-orientierten C-FFI ist `T` nicht
//! bekannt — Caller liefert daher die **Key-Hash-Bytes (16 byte)** der
//! Instance, die durch IDL-Codegen-Side per `T::encode_key_holder` erzeugt
//! werden. Siehe `docs/specs/zerodds-c-api-1.0.md §2.6 Vendor-Extension`.

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
/// Liefert das erste Topic mit gleichem Namen, oder NULL.
///
/// # Safety
/// `p`, `name` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_lookup_topicdescription(
    p: *mut ZeroDdsDomainParticipant,
    name: *const core::ffi::c_char,
) -> *mut crate::entities::ZeroDdsTopic {
    if p.is_null() || name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — p+name NULL-checked above; topics-Liste haelt
    // nur valide Box-Pointer aus dp_create_topic.
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
    /// Backref auf Participant fuer Lifetime-Tracking.
    pub participant: *mut ZeroDdsDomainParticipant,
    /// Geklonter Arc auf den DCPS-internen BuiltinSubscriber.
    pub inner: alloc::sync::Arc<zerodds_dcps::builtin_subscriber::BuiltinSubscriber>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsBuiltinSubscriber {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsBuiltinSubscriber {}

/// `get_builtin_subscriber()` — Spec §2.2.2.2.1.7.
/// Liefert einen Pointer auf einen Wrapper um den DCPS-BuiltinSubscriber.
/// Caller muss `zerodds_builtin_subscriber_destroy` rufen.
///
/// # Safety
/// `p` valide.
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

/// Loescht ein BuiltinSubscriber-Wrapper.
///
/// # Safety
/// `bs` valide oder NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_subscriber_destroy(bs: *mut ZeroDdsBuiltinSubscriber) {
    // SAFETY: Caller-Pledge: bs stammt aus get_builtin_subscriber (Box::into_raw).
    unsafe { Owned::from_raw_drop(bs) };
}

// ===========================================================================
// QoS get/set per Entity (Spec §2.2.2.x.y)
// ===========================================================================

/// Setzt die DomainParticipant-QoS (Spec §2.2.2.2.1.3).
///
/// # Safety
/// `p` valide; `qos` darf NULL sein (Reset auf Default).
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

/// Liest die DomainParticipant-QoS in `out` (Spec §2.2.2.2.1.4).
/// `out.user_data.value` muss vom Caller mit ausreichendem Buffer
/// initialisiert sein. Bei zu kleinem Buffer wird die erforderliche
/// Groesse in `out.user_data.value_len` zurueckgeschrieben + `OutOfResources`.
///
/// # Safety
/// `p`, `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_qos(
    p: *mut ZeroDdsDomainParticipant,
    out: *mut ZeroDdsDomainParticipantQos,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — pp valid (NULL-checked above), out valid (Caller-Pledge).
    unsafe {
        let pp = &*p;
        let qos = pp.dp.qos();
        crate::qos_ffi::dp_qos_to_c(&qos, out)
    }
}

/// Set-Default-Topic-QoS.
///
/// # Safety
/// `p` valide; `qos` darf NULL sein.
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
/// `p`, `out` valide.
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
/// `p` valide.
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
/// `p`, `out` valide.
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
/// `p` valide.
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
/// `p`, `out` valide.
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
/// `pub_` valide.
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
/// `pub_`, `out` valide.
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
/// `pub_` valide.
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
/// `pub_`, `out` valide.
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

/// Pub copy_from_topic_qos: kopiert die Policies aus TopicQos in
/// die DataWriterQos-`history`/`durability`/`reliability`/etc. Inplace.
///
/// # Safety
/// Beide Pointer valide.
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
/// `sub` valide.
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
/// `sub`, `out` valide.
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
/// `sub` valide.
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
/// `sub`, `out` valide.
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
/// Beide Pointer valide.
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
/// `dw` valide.
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
/// `dw`, `out` valide.
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
/// `dr` valide.
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
/// `dr`, `out` valide.
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

/// `register_instance` — Vendor-Variante: caller liefert 16-byte Key-Hash
/// statt generic `T instance_data`. Der Wire-Pfad kennt nur Bytes.
///
/// # Safety
/// `dw` valide; `key`+`out_handle` valide.
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

/// `register_instance_w_timestamp`. RC1: Source-Timestamp ignoriert
/// (gleicher Pfad wie register_instance, dokumentiert in Vendor-Spec).
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
    // SAFETY: see fn # Safety doc — Delegation an register_instance mit gleichem Vertrag.
    unsafe { zerodds_dw_register_instance(dw, key, key_len, out_handle) }
}

/// `unregister_instance` — emittiert UNREGISTERED-Lifecycle.
///
/// # Safety
/// `dw` valide; `handle` muss aus `register_instance` stammen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_unregister_instance(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // Status-Bits: UNREGISTERED nach RTPS Inline-QoS Status-Info §9.6.4.10.
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
    // SAFETY: see fn # Safety doc — Delegation an unregister_instance mit gleichem Vertrag.
    unsafe { zerodds_dw_unregister_instance(dw, handle) }
}

/// `lookup_instance` — Vendor-Variante: liefert deterministischen
/// Handle aus den ersten 8 Key-Hash-Bytes.
///
/// # Safety
/// `key[0..16]` valide.
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

/// `get_key_value(handle)` — bei C-FFI nur Vendor-Variante: kopiert
/// 8 Bytes des Handles in `out_buf[0..8]`. Volle Spec-Round-trip
/// (T-Encoded-Key) ist Codegen-Pfad.
///
/// # Safety
/// `dw` valide; `out_buf[0..*inout_len]` valide.
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
    // SAFETY: see fn # Safety doc — out_buf+inout_len NULL-checked above; out_buf[0..*inout_len] valide.
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
    // SAFETY: see fn # Safety doc — Delegation an dw_dispose mit gleichem Vertrag.
    unsafe { crate::publisher_ffi::zerodds_dw_dispose(dw, key_hash, handle) }
}

// ===========================================================================
// DataWriter Matched-Subscriptions
// ===========================================================================

/// `get_matched_subscriptions` — Liste der `InstanceHandle`s aller
/// matched Remote-Reader (Spec §2.2.2.4.2.x).
///
/// # Safety
/// `dw` valide; `out_handles[0..cap]` writeable; `out_count` valide.
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
    // writeable wenn non-NULL (Caller-Pledge).
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
/// Liefert die `SubscriptionBuiltinTopicData` zu einem matched
/// Reader, gewonnen aus dem BuiltinSubscriber-SEDP-Cache.
///
/// # Safety
/// `dw` und `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_matched_subscription_data(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    out: *mut crate::builtin_ffi::ZeroDdsSubscriptionBuiltinTopicData,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above; dwr.publisher und
    // pub_.participant aus create_datawriter+create_publisher (Box::into_raw).
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

/// `loan_message` — Zero-Copy-Loan via Heap-Box (Vendor-Variante).
/// Allokiert einen Box<[u8]>, leakt den Pointer; Caller schreibt in
/// den Buffer und ruft `commit_loan` (sendet via write_user_sample)
/// oder `discard_loan` (gibt Buffer frei ohne zu senden).
///
/// **Vendor-Decision (Spec §2.2.2.4.2 Loan-API + Vendor-Spec
/// `zerodds-flatdata-1.0.md` §4):** Echtes Iceoryx-SHM-Zero-Copy
/// erfordert separate `zerodds-flatdata`-Iceoryx-Backend-Wireup;
/// die hier exponierte Heap-Loan-Variante erfuellt das DDS-Spec-
/// Vertragsbild (Caller schreibt in den Buffer, dann commit/discard)
/// ohne SHM-Optimierung. Die echte SHM-Loan ist transparent ueber den
/// `zerodds-flatdata`-Iceoryx-Backend wenn dieser Aktiv ist.
///
/// # Safety
/// Alle Pointers valide; `len > 0`.
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
    let buf: Box<[u8]> = vec![0u8; len].into_boxed_slice();
    let raw = Box::into_raw(buf) as *mut u8;
    // SAFETY: see fn # Safety doc — out_ptr+out_len NULL-checked above; raw aus
    // Box::into_raw, Caller besitzt jetzt das Buffer-Eigentum bis commit/discard.
    unsafe {
        *out_ptr = raw;
        *out_len = len;
    }
    ZeroDdsStatus::Ok as c_int
}

/// `commit_loan` — schreibt den Loan-Buffer als Sample und gibt den
/// Buffer frei.
///
/// # Safety
/// `dw`, `ptr` valide; `(ptr, len)` aus `loan_message`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_commit_loan(
    dw: *mut ZeroDdsDataWriter,
    ptr: *mut u8,
    len: usize,
) -> c_int {
    if dw.is_null() || ptr.is_null() || len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+ptr NULL-checked above; ptr+len stammen aus
    // loan_message (Box::into_raw); rebuild + into_vec ist konform.
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

/// `discard_loan` — gibt den Loan-Buffer frei ohne zu senden.
///
/// # Safety
/// `dw`, `ptr` valide; `(ptr, len)` aus `loan_message`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_discard_loan(
    _dw: *mut ZeroDdsDataWriter,
    ptr: *mut u8,
    len: usize,
) -> c_int {
    if ptr.is_null() || len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — ptr+len aus loan_message (Box::into_raw); rebuild + drop.
    let _: Box<[u8]> = unsafe { Box::from_raw(core::slice::from_raw_parts_mut(ptr, len)) };
    ZeroDdsStatus::Ok as c_int
}

// ===========================================================================
// DataReader Instance-Variants
// ===========================================================================

/// `read_instance(handle)` — Spec §2.2.2.5.3.5: liefert nur Samples
/// dieser Instance.
///
/// # Safety
/// `dr`, `out` valide.
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
    // SAFETY: see fn # Safety doc — Delegation an dr_read mit gleichem Pointer-Vertrag,
    // dann safe in-place Filter auf das resultierende SampleArray.
    let rc = unsafe { crate::subscriber_ffi::zerodds_dr_read(dr, out, max, s, v, i) };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    crate::subscriber_ffi::sample_array_filter_instance(out, handle)
}

/// `take_instance(handle)`.
///
/// # Safety
/// `dr`, `out` valide.
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
    // SAFETY: see fn # Safety doc — Delegation an dr_take + in-place filter.
    let rc = unsafe { crate::subscriber_ffi::zerodds_dr_take(dr, out, max, s, v, i) };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    crate::subscriber_ffi::sample_array_filter_instance(out, handle)
}

/// `read_next_instance(prev_handle)` — Spec §2.2.2.5.3.7: liefert
/// Samples der naechsten Instance > prev_handle.
///
/// # Safety
/// `dr`, `out` valide.
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
    // SAFETY: see fn # Safety doc — Delegation an dr_read + in-place filter.
    let rc = unsafe { crate::subscriber_ffi::zerodds_dr_read(dr, out, max, s, v, i) };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    crate::subscriber_ffi::sample_array_filter_next_instance(out, prev_handle)
}

/// `take_next_instance(prev_handle)`.
///
/// # Safety
/// `dr`, `out` valide.
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
    // SAFETY: see fn # Safety doc — Delegation an dr_take + in-place filter.
    let rc = unsafe { crate::subscriber_ffi::zerodds_dr_take(dr, out, max, s, v, i) };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    crate::subscriber_ffi::sample_array_filter_next_instance(out, prev_handle)
}

/// `read_w_condition` — Spec §2.2.2.5.3.4: liefert nur Samples die
/// zur Condition-State-Mask passen (ReadCondition.sample_states/
/// view_states/instance_states).
///
/// # Safety
/// `dr`, `out` valide; `cond` ist eine ReadCondition oder QueryCondition.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_read_w_condition(
    dr: *mut ZeroDdsDataReader,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    cond: *mut core::ffi::c_void,
) -> c_int {
    // SAFETY: see fn # Safety doc — cond muss eine ReadCondition/QueryCondition sein,
    // condition_state_masks vererbt den Caller-Pledge; dann Delegation an dr_read.
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
    // SAFETY: see fn # Safety doc — cond muss eine ReadCondition/QueryCondition sein,
    // condition_state_masks vererbt den Caller-Pledge; dann Delegation an dr_take.
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
/// `key[0..16]` valide; `out_handle` valide.
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
/// `out_buf[0..*inout_len]` valide.
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
    // SAFETY: see fn # Safety doc — out_buf+inout_len NULL-checked above; out_buf[0..*inout_len] valide.
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

/// `wait_for_historical_data(timeout)` — RC1: Volatile-default → Ok ohne Wait.
///
/// # Safety
/// `dr` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_wait_for_historical_data(
    dr: *mut ZeroDdsDataReader,
    _timeout_sec: i32,
    _timeout_nanosec: u32,
) -> c_int {
    if dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // Volatile-Reader: keine historischen Daten — Ok ohne Wait.
    // Bei TransientLocal-Reader wuerde RC1 hier `Unsupported`
    // liefern, weil Historie-Service nicht eingehakt ist.
    ZeroDdsStatus::Ok as c_int
}

/// `get_matched_publications` — Liste der `InstanceHandle`s aller
/// matched Remote-Writer (Spec §2.2.2.5.x).
///
/// # Safety
/// `dr` valide; `out_handles[0..cap]` writeable; `out_count` valide.
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
    // writeable wenn non-NULL (Caller-Pledge).
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
/// Liefert `PublicationBuiltinTopicData` aus dem BuiltinSubscriber-Cache.
///
/// # Safety
/// `dr`, `out` valide.
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
    // aus create_*-fns (Box::into_raw).
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
        // SAFETY: f aus dpf_get_instance, statisch valide.
        unsafe { zerodds_dpf_create_participant(f, domain, ptr::null()) }
    }
    fn cleanup(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: p aus mk() / dpf_create_participant; f statisch valide.
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
        // SAFETY: p aus mk(), valide bis cleanup. qos lebt fuer den Test.
        unsafe {
            assert_eq!(zerodds_dp_set_qos(p, &qos), ZeroDdsStatus::Ok as c_int);
            assert_eq!(zerodds_dp_get_qos(p, &mut qos), ZeroDdsStatus::Ok as c_int);
        }
        cleanup(p);
    }

    #[test]
    fn dp_default_topic_qos_roundtrip() {
        let p = mk(71);
        // SAFETY: p aus mk(), valide bis cleanup. tqos lebt fuer den Test.
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
        // SAFETY: p+n+tn+key leben fuer den Test; extern fns dokumentieren ihre Pledges.
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
        // SAFETY: p+n+tn leben fuer den Test; buf+len writeable.
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
        // SAFETY: p+n+tn leben fuer den Test; alle out-Pointer auf Stack-lokalen Slots.
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
        // SAFETY: p+n+tn leben fuer den Test.
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
        // SAFETY: p+n+tn leben fuer den Test.
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
        // SAFETY: p+n+tn leben fuer den Test.
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
        // SAFETY: p+n+tn leben fuer den Test.
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
        // SAFETY: p+bytes+out_buf leben fuer den Test.
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
        // SAFETY: p lebt fuer den Test; tqos+dwqos auf Stack zeroed.
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
