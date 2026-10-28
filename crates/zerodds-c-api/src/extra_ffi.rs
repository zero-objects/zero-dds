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
use std::time::Duration;

use crate::ZeroDdsStatus;
use crate::entities::{
    ZeroDdsDataReader, ZeroDdsDataWriter, ZeroDdsDomainParticipant, ZeroDdsPublisher,
    ZeroDdsSubscriber,
};
#[allow(unused_imports)]
use crate::ffi_helpers::{Borrowed, BytesIn, BytesOut, OutPtr, Owned, copy_list_out, status};
use crate::qos_ffi::{
    ZeroDdsDataReaderQos, ZeroDdsDataWriterQos, ZeroDdsDomainParticipantQos, ZeroDdsPublisherQos,
    ZeroDdsSubscriberQos, ZeroDdsTopicQos, dr_qos_from_c_checked, dw_qos_from_c, pub_qos_from_c,
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

/// `lookup_topicdescription(name, index)` — DDS-XTypes 1.3 §7.6.4.2: "The
/// lookup_topicdescription operation shall accept an optional in unsigned
/// long argument called index". The topics of one name are iterated in the
/// order they were created; `index` 0 is the first, which is what
/// `zerodds_dp_lookup_topicdescription` answers. NULL for an index past the
/// last.
///
/// # Safety
/// `p`, `name` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_lookup_topicdescription_at(
    p: *mut ZeroDdsDomainParticipant,
    name: *const core::ffi::c_char,
    index: u32,
) -> *mut crate::entities::ZeroDdsTopic {
    if p.is_null() || name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: p+name NULL-checked above; the topics list holds only valid
    // box pointers from dp_create_topic.
    unsafe {
        let pp = &*p;
        let Ok(name_str) = std::ffi::CStr::from_ptr(name).to_str() else {
            return ptr::null_mut();
        };
        if let Ok(list) = pp.topics.lock() {
            return list
                .iter()
                .copied()
                .filter(|t| !t.is_null() && (**t).name == name_str)
                .nth(index as usize)
                .unwrap_or(ptr::null_mut());
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

/// DDS 1.4 §2.2.2.1.1.1: a NULL QoS passed to `set_qos` is `*_QOS_DEFAULT`,
/// "the current default QoS set in the Entity's factory". Without a
/// factory, the specification's default.
pub(crate) fn current_default<Q: Clone + Default>(slot: Option<&std::sync::Mutex<Q>>) -> Q {
    slot.and_then(|m| m.lock().ok().map(|g| g.clone()))
        .unwrap_or_default()
}

/// Sets the DomainParticipant QoS (Spec §2.2.2.2.1.3).
///
/// # Safety
/// `p` valid; `qos` may be NULL: the factory's default participant QoS
/// (`PARTICIPANT_QOS_DEFAULT`).
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
            current_default(Some(
                &crate::entities::ZeroDdsDomainParticipantFactory::instance()
                    .default_participant_qos,
            ))
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
        crate::qos_ffi::pub_qos_to_c(&qos, out, &pp.default_pub_partition_out)
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
        crate::qos_ffi::sub_qos_to_c(&qos, out, &pp.default_sub_partition_out)
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
            current_default(pp.participant.as_ref().map(|dp| &dp.default_publisher_qos))
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
        crate::qos_ffi::pub_qos_to_c(&qos, out, &pp.partition_out)
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
        crate::qos_ffi::dw_qos_to_c(&qos, out, &pp.partition_out)
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
            current_default(sb.participant.as_ref().map(|dp| &dp.default_subscriber_qos))
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
        crate::qos_ffi::sub_qos_to_c(&qos, out, &sb.partition_out)
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
            match dr_qos_from_c_checked(qos) {
                Ok(q) => q,
                Err(status) => return status as c_int,
            }
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
        crate::qos_ffi::dr_qos_to_c(&qos, out, &sb.partition_out)
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
    // SAFETY: see fn # Safety doc — dw NULL-checked above; qos NULL-tolerant;
    // dwr.publisher from pub_create_datawriter.
    unsafe {
        let dwr = &*dw;
        let new_qos = if qos.is_null() {
            current_default(dwr.publisher.as_ref().map(|p| &p.default_dw_qos))
        } else {
            dw_qos_from_c(qos)
        };
        let Ok(mut current) = dwr.qos.lock() else {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        };
        // DDS 1.4 §2.2.3: a C writer is enabled from its creation. It took
        // any QoS and stored it: an immutable policy "changed" without
        // effect, and a changeable one never reached the wire.
        // DDS 1.4 §2.2.2.1.1.2: an immutable policy can be set "before the
        // entity is made enabled"; a writer not registered yet announces
        // the QoS it has when it is enabled.
        let enabled = dwr.lifecycle.is_registered();
        if enabled && zerodds_dcps::qos::immutable_writer_change(&current, &new_qos).is_some() {
            return ZeroDdsStatus::ImmutablePolicy as c_int;
        }
        if enabled && !dwr.publisher.is_null() {
            let pp = &*dwr.publisher;
            let update = writer_qos_update(pp, &new_qos);
            if update != writer_qos_update(pp, &current)
                && dwr.rt.update_user_writer_qos(dwr.eid, &update).is_err()
            {
                return ZeroDdsStatus::Error as c_int;
            }
        }
        *current = new_qos;
    }
    ZeroDdsStatus::Ok as c_int
}

/// The changeable policies of a C writer the runtime holds, with the
/// publisher's PARTITION and GROUP_DATA where the writer sets none.
fn writer_qos_update(
    pp: &crate::entities::ZeroDdsPublisher,
    qos: &zerodds_dcps::qos::DataWriterQos,
) -> zerodds_dcps::runtime::WriterQosUpdate {
    let scoped = crate::publisher_ffi::publisher_scoped(pp, qos);
    zerodds_dcps::runtime::WriterQosUpdate {
        deadline: qos.deadline,
        latency_budget: qos.latency_budget,
        lifespan: qos.lifespan,
        ownership_strength: qos.ownership_strength.value,
        transport_priority: qos.transport_priority.value,
        user_data: qos.user_data.value.clone(),
        own: zerodds_dcps::runtime::GroupTopicQos {
            partition: scoped.partition,
            group_data: scoped.group_data,
            topic_data: qos.topic_data.value.clone(),
        },
    }
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
        crate::qos_ffi::dw_qos_to_c(&qos, out, &dwr.partition_out)
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
    // SAFETY: see fn # Safety doc — dr NULL-checked above; qos NULL-tolerant;
    // drr.subscriber from sub_create_datareader.
    unsafe {
        let drr = &*dr;
        let new_qos = if qos.is_null() {
            current_default(drr.subscriber.as_ref().map(|s| &s.default_dr_qos))
        } else {
            match dr_qos_from_c_checked(qos) {
                Ok(q) => q,
                Err(status) => return status as c_int,
            }
        };
        let Ok(mut current) = drr.qos.lock() else {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        };
        // See `zerodds_dw_set_qos`.
        let enabled = drr.lifecycle.is_registered();
        if enabled && zerodds_dcps::qos::immutable_reader_change(&current, &new_qos).is_some() {
            return ZeroDdsStatus::ImmutablePolicy as c_int;
        }
        if enabled && !drr.subscriber.is_null() {
            let sb = &*drr.subscriber;
            let update = reader_qos_update(sb, &new_qos);
            if update != reader_qos_update(sb, &current)
                && drr.rt.update_user_reader_qos(drr.eid, &update).is_err()
            {
                return ZeroDdsStatus::Error as c_int;
            }
        }
        drr.rt.set_user_reader_time_based_filter(
            drr.eid,
            crate::subscriber_ffi::time_based_filter_nanos(&new_qos),
        );
        *current = new_qos;
    }
    ZeroDdsStatus::Ok as c_int
}

/// The reader half of [`writer_qos_update`].
fn reader_qos_update(
    sb: &crate::entities::ZeroDdsSubscriber,
    qos: &zerodds_dcps::qos::DataReaderQos,
) -> zerodds_dcps::runtime::ReaderQosUpdate {
    let scoped = crate::subscriber_ffi::subscriber_scoped(sb, qos);
    zerodds_dcps::runtime::ReaderQosUpdate {
        deadline: qos.deadline,
        time_based_filter: qos.time_based_filter,
        latency_budget: qos.latency_budget,
        user_data: qos.user_data.value.clone(),
        own: zerodds_dcps::runtime::GroupTopicQos {
            partition: scoped.partition,
            group_data: scoped.group_data,
            topic_data: qos.topic_data.value.clone(),
        },
    }
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
        crate::qos_ffi::dr_qos_to_c(&qos, out, &drr.partition_out)
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
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    if key_len != 16 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — Newtypes encapsulate raw-pointer validity pledge.
    status(unsafe {
        (|| -> Result<(), ZeroDdsStatus> {
            let dw = Borrowed::from_raw(dw).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let key_in = BytesIn::from_raw(key, 16).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let out = OutPtr::from_raw(out_handle).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let mut full_key = [0u8; 16];
            full_key.copy_from_slice(&key_in);
            let full_key = crate::publisher_ffi::instance_key_of(dw.eid, full_key);
            let limit = dw
                .qos
                .lock()
                .map_err(|_| ZeroDdsStatus::Error)?
                .resource_limits
                .max_instances;
            let handle = dw
                .instances
                .ensure_registered_limited(full_key, limit)
                .ok_or(ZeroDdsStatus::OutOfResources)?;
            out.write(handle.as_raw());
            Ok(())
        })()
    })
}

/// `register_instance_w_timestamp`. A registration sends nothing, so the
/// timestamp is only checked: DDS 1.4 §2.3.2, `ts_nanosec` below 10^9.
///
/// # Safety
/// Wie `register_instance`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_register_instance_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key: *const u8,
    key_len: usize,
    _ts_sec: i32,
    ts_nanosec: u32,
    out_handle: *mut u64,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    if ts_nanosec >= 1_000_000_000 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
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
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    // SAFETY: the caller's contract.
    unsafe { unregister_at(dw, handle, None) }
}

/// [`zerodds_dw_unregister_instance`] with the marker's source timestamp
/// (DDS 1.4 §2.2.2.4.2.8 `unregister_instance_w_timestamp`).
///
/// # Safety
/// As `zerodds_dw_unregister_instance`.
unsafe fn unregister_at(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    source_timestamp: Option<zerodds_rtps::header_extension::HeTimestamp>,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // Status bits: UNREGISTERED per RTPS inline-QoS status info §9.6.4.10.
    const STATUS_UNREGISTERED: u32 = 0x0000_0002;
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
    let dwr = unsafe { &*dw };
    let instance = zerodds_dcps::instance_handle::InstanceHandle::from_raw(handle);
    let Some(k) = dwr.instances.key_hash_for_handle(instance) else {
        return ZeroDdsStatus::BadHandle as c_int;
    };
    let (rt, eid) = (dwr.rt.clone(), dwr.eid);
    match rt.write_user_lifecycle(eid, k, None, STATUS_UNREGISTERED, source_timestamp) {
        Ok(()) => {
            dwr.instances.unregister(instance, None);
            ZeroDdsStatus::Ok as c_int
        }
        Err(e) => crate::write_status(&e),
    }
}

/// `unregister_instance_w_timestamp`: the marker carries the timestamp as
/// its source timestamp (DDS 1.4 §2.2.2.4.2.8); `ts_nanosec` of 10^9 or
/// more is BAD_PARAMETER (§2.3.2). The timestamp was dropped.
///
/// # Safety
/// Wie `unregister_instance`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_unregister_instance_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    if ts_nanosec >= 1_000_000_000 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let ts = zerodds_dcps::time::time_to_he_timestamp(zerodds_dcps::Time::new(ts_sec, ts_nanosec));
    // SAFETY: see fn # Safety doc — the same contract.
    unsafe { unregister_at(dw, handle, Some(ts)) }
}

/// Returns the registered opaque handle, or HANDLE_NIL for an unknown key.
///
/// # Safety
/// `key[0..16]` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_lookup_instance(
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
            let dw = Borrowed::from_raw(dw).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let key_in = BytesIn::from_raw(key, 16).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let out = OutPtr::from_raw(out_handle).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let mut full_key = [0u8; 16];
            full_key.copy_from_slice(&key_in);
            let full_key = crate::publisher_ffi::instance_key_of(dw.eid, full_key);
            out.write(dw.instances.lookup(&full_key).map_or(0, |h| h.as_raw()));
            Ok(())
        })()
    })
}

/// Returns a stored serialized key holder. An instance known by its hash
/// alone has its key from the hash where the type spells it out (DDSI-RTPS
/// 2.5 §9.6.4.8 Step 5.1); where it does not, or the writer has no type,
/// the answer is `BadParameter`, the one error DDS 1.4 §2.2.2.4.2.9 names.
///
/// # Safety
/// `dw` valid; `out_buf[0..*inout_len]` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_key_value(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    out_buf: *mut u8,
    inout_len: *mut usize,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    if dw.is_null() || out_buf.is_null() || inout_len.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: live writer and output storage pledged by the caller.
    unsafe {
        let dwr = &*dw;
        copy_stored_key_holder(&dwr.instances, handle, out_buf, inout_len, |hash| {
            dwr.rt.user_writer_key_holder_from_hash(dwr.eid, hash)
        })
    }
}

/// # Safety
/// `out_buf[0..*inout_len]` and `inout_len` are valid writable storage.
unsafe fn copy_stored_key_holder(
    tracker: &zerodds_dcps::instance_tracker::InstanceTracker,
    handle: u64,
    out_buf: *mut u8,
    inout_len: *mut usize,
    derive_from_hash: impl FnOnce([u8; 16]) -> Option<Vec<u8>>,
) -> c_int {
    let handle = zerodds_dcps::instance_handle::InstanceHandle::from_raw(handle);
    // A known handle and a hash-only registration are different answers:
    // unknown is `BadHandle`; known by its hash alone, the key is read out of
    // the hash where the hash spells it out, and refused otherwise instead
    // of inventing a holder.
    let Some(hash) = tracker.key_hash_for_handle(handle) else {
        return ZeroDdsStatus::BadHandle as c_int;
    };
    let stored = tracker.get_key_holder(handle).filter(|h| !h.is_empty());
    let Some(holder) = stored.or_else(|| derive_from_hash(hash)) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    // SAFETY: caller guarantees the output storage and length pointer.
    unsafe {
        if *inout_len < holder.len() {
            *inout_len = holder.len();
            return ZeroDdsStatus::OutOfResources as c_int;
        }
        ptr::copy_nonoverlapping(holder.as_ptr(), out_buf, holder.len());
        *inout_len = holder.len();
    }
    ZeroDdsStatus::Ok as c_int
}

/// `dispose_w_timestamp`: the marker carries the timestamp as its source
/// timestamp (DDS 1.4 §2.2.2.4.2.11); `ts_nanosec` of 10^9 or more is
/// BAD_PARAMETER (§2.3.2). The timestamp was dropped.
///
/// # Safety
/// Wie `dispose`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_dispose_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key_hash: *const u8,
    handle: u64,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    if ts_nanosec >= 1_000_000_000 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let ts = zerodds_dcps::time::time_to_he_timestamp(zerodds_dcps::Time::new(ts_sec, ts_nanosec));
    // SAFETY: see fn # Safety doc — the same contract.
    unsafe { crate::publisher_ffi::dispose_at(dw, key_hash, handle, Some(ts)) }
}

// ===========================================================================
// DataWriter Matched-Subscriptions
// ===========================================================================

/// `get_matched_subscriptions` — list of the `InstanceHandle`s of all
/// matched remote readers (Spec §2.2.2.4.2.x).
///
/// The first `min(len, cap)` handles are written into `out_handles`,
/// `*out_count` is set to the **total** number of matched readers, and the
/// return value is `OutOfResources` when `len > cap` (so a caller retries with
/// a buffer of `*out_count`), `Ok` otherwise. A NULL `out_handles` with
/// `cap == 0` asks for the length alone (`docs/specs/zerodds-c-api-1.0.md`
/// §2.8).
///
/// # Safety
/// `dw`, `out_count` valid; `out_handles[0..cap]` writeable, or `out_handles`
/// NULL exactly when `cap == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_matched_subscriptions(
    dw: *mut ZeroDdsDataWriter,
    out_handles: *mut u64,
    out_count: *mut usize,
    cap: usize,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    if dw.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out_count NULL-checked above; the
    // buffer contract is enforced by `copy_list_out`.
    unsafe {
        let dwr = &*dw;
        let handles: Vec<u64> = dwr
            .rt
            .user_writer_matched_subscription_handles(dwr.eid)
            .iter()
            .map(|h| h.as_raw())
            .collect();
        copy_list_out(out_handles, cap, &handles, out_count)
    }
}

/// `get_matched_subscription_data(handle)` — DDS 1.4 §2.2.2.4.2. Fills
/// `out` with the DCPSSubscription data of the reader `handle` names, as
/// it was last announced: a discovered reader's or one of this
/// participant's. The caller releases it with
/// `zerodds_subscription_builtin_topic_data_free`.
///
/// `ZERODDS_BAD_PARAMETER` for a handle that names no reader this writer
/// is matched with, as the specification has it. It answered
/// `ZERODDS_NO_DATA` for that, answered for a discovered reader the writer
/// was not matched with, and did not find a reader of the writer's own
/// participant that `get_matched_subscriptions` listed.
///
/// # Safety
/// `dw` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_matched_subscription_data(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    out: *mut crate::builtin_ffi::ZeroDdsSubscriptionBuiltinTopicData,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
    let dwr = unsafe { &*dw };
    let Some(data) = dwr.rt.user_writer_matched_subscription_data(
        dwr.eid,
        zerodds_dcps::instance_handle::InstanceHandle::from_raw(handle),
    ) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    write_subscription_data(
        out,
        &zerodds_dcps::builtin_topics::SubscriptionBuiltinTopicData::from_wire(&data),
    )
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
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
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
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
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
        Err(e) => crate::write_status(&e),
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

use crate::subscriber_ffi::InstanceSelect;

/// [`crate::subscriber_ffi::read_or_take_selected`] with its selection
/// spelled out.
///
/// # Safety
/// `dr` and `out` valid or null.
unsafe fn read_selected(
    dr: *mut ZeroDdsDataReader,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    states: (u32, u32, u32),
    instance: InstanceSelect,
    take: bool,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe {
        crate::subscriber_ffi::read_or_take_selected(
            dr,
            out,
            max,
            crate::subscriber_ffi::Selection { states, instance },
            take,
        )
    }
}

/// Read or take through a ReadCondition or QueryCondition: its state masks and, for a
/// QueryCondition, its expression (DDS 1.4 §2.2.2.5.3.4, §2.2.2.5.9).
///
/// # Safety
/// `dr`, `out` valid or null; `cond` a condition or null.
unsafe fn read_with_condition(
    dr: *mut ZeroDdsDataReader,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    cond: *mut core::ffi::c_void,
    instance: InstanceSelect,
    take: bool,
) -> c_int {
    // SAFETY: the caller's contract; condition_state_masks inherits it.
    unsafe {
        let Some(states) = crate::condition_ffi::condition_state_masks(cond) else {
            return ZeroDdsStatus::BadParameter as c_int;
        };
        let query = match crate::condition_ffi::condition_query(cond) {
            Ok(q) => q,
            Err(status) => return status as c_int,
        };
        crate::subscriber_ffi::read_or_take_selected_where(
            dr,
            out,
            max,
            crate::subscriber_ffi::Selection { states, instance },
            query.as_ref(),
            take,
        )
    }
}

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
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe { read_selected(dr, out, max, (s, v, i), InstanceSelect::Is(handle), false) }
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
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe { read_selected(dr, out, max, (s, v, i), InstanceSelect::Is(handle), true) }
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
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe {
        read_selected(
            dr,
            out,
            max,
            (s, v, i),
            InstanceSelect::After(prev_handle),
            false,
        )
    }
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
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe {
        read_selected(
            dr,
            out,
            max,
            (s, v, i),
            InstanceSelect::After(prev_handle),
            true,
        )
    }
}

/// `read_w_condition` — Spec §2.2.2.5.3.4: returns only samples that
/// match the condition state mask (ReadCondition.sample_states/
/// view_states/instance_states) and, for a QueryCondition, its expression. A
/// QueryCondition whose expression names a field needs the reader's type
/// (`zerodds_dr_set_type_objects`); without one the call returns
/// `PRECONDITION_NOT_MET` instead of passing every sample.
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
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe { read_with_condition(dr, out, max, cond, InstanceSelect::Any, false) }
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
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe { read_with_condition(dr, out, max, cond, InstanceSelect::Any, true) }
}

/// `read_next_instance_w_condition` — DDS 1.4 §2.2.2.5.3.12: the samples of the
/// next instance after `prev_handle` that match the state masks of `cond`.
///
/// # Safety
/// `dr`, `out` valid; `cond` is a ReadCondition or QueryCondition.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_read_next_instance_w_condition(
    dr: *mut ZeroDdsDataReader,
    prev_handle: u64,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    cond: *mut core::ffi::c_void,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe {
        read_with_condition(
            dr,
            out,
            max,
            cond,
            InstanceSelect::After(prev_handle),
            false,
        )
    }
}

/// `take_next_instance_w_condition` — like [`zerodds_dr_read_next_instance_w_condition`],
/// removing the samples.
///
/// # Safety
/// Same as [`zerodds_dr_read_next_instance_w_condition`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_take_next_instance_w_condition(
    dr: *mut ZeroDdsDataReader,
    prev_handle: u64,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    cond: *mut core::ffi::c_void,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe { read_with_condition(dr, out, max, cond, InstanceSelect::After(prev_handle), true) }
}

/// `read_instance_w_condition`: the samples of the instance `handle` that match the state
/// masks and, for a QueryCondition, the expression of `cond`. DDS 1.4 has no such operation;
/// DDS-PSM-Cxx 1.0 `select().instance(h).content(query)` asks for it.
///
/// # Safety
/// `dr`, `out` valid; `cond` is a ReadCondition or QueryCondition.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_read_instance_w_condition(
    dr: *mut ZeroDdsDataReader,
    handle: u64,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    cond: *mut core::ffi::c_void,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe { read_with_condition(dr, out, max, cond, InstanceSelect::Is(handle), false) }
}

/// `take_instance_w_condition`, removing the samples.
///
/// # Safety
/// Same as [`zerodds_dr_read_instance_w_condition`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_take_instance_w_condition(
    dr: *mut ZeroDdsDataReader,
    handle: u64,
    out: *mut crate::subscriber_ffi::ZeroDdsSampleArray,
    max: usize,
    cond: *mut core::ffi::c_void,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe { read_with_condition(dr, out, max, cond, InstanceSelect::Is(handle), true) }
}

/// `lookup_instance` (Reader).
///
/// # Safety
/// `key[0..16]` valid; `out_handle` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_lookup_instance(
    dr: *mut ZeroDdsDataReader,
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
            let dr = Borrowed::from_raw(dr).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let key_in = BytesIn::from_raw(key, 16).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let out = OutPtr::from_raw(out_handle).map_err(|_| ZeroDdsStatus::BadParameter)?;
            let mut full_key = [0u8; 16];
            full_key.copy_from_slice(&key_in);
            out.write(dr.instances.lookup(&full_key).map_or(0, |h| h.as_raw()));
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
    dr: *mut ZeroDdsDataReader,
    handle: u64,
    out_buf: *mut u8,
    inout_len: *mut usize,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    if dr.is_null() || out_buf.is_null() || inout_len.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: live reader and output storage pledged by the caller.
    unsafe {
        let drr = &*dr;
        copy_stored_key_holder(&drr.instances, handle, out_buf, inout_len, |hash| {
            drr.rt.user_reader_key_holder_from_hash(drr.eid, hash)
        })
    }
}

/// `wait_for_historical_data(max_wait)` (DDS 1.4 §2.2.2.5.3): blocks until
/// the reader holds the historical data of every writer it is matched
/// with — each reliable writer's HEARTBEAT arrived and everything it
/// announced is received — or `max_wait` passes (`ZERODDS_TIMEOUT`). A
/// VOLATILE reader returns at once. It returned OK at once for every
/// reader.
///
/// # Safety
/// `dr` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_wait_for_historical_data(
    dr: *mut ZeroDdsDataReader,
    timeout_sec: i32,
    timeout_nanosec: u32,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    if dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — dr NULL-checked above.
    let drr = unsafe { &*dr };
    let volatile = drr
        .qos
        .lock()
        .map(|q| {
            matches!(
                q.durability.kind,
                zerodds_dcps::qos::DurabilityKind::Volatile
            )
        })
        .unwrap_or(true);
    if volatile {
        return ZeroDdsStatus::Ok as c_int;
    }
    let max_wait = Duration::from_secs(u64::try_from(timeout_sec).unwrap_or(0))
        + Duration::from_nanos(u64::from(timeout_nanosec));
    let deadline = std::time::Instant::now() + max_wait;
    loop {
        let seen = drr.rt.reader_progress_generation();
        if drr.rt.user_reader_historical_data_received(drr.eid) {
            return ZeroDdsStatus::Ok as c_int;
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return ZeroDdsStatus::Timeout as c_int;
        }
        drr.rt.wait_reader_progress(seen, deadline - now);
    }
}

/// `get_matched_publications` — list of the `InstanceHandle`s of all
/// matched remote writers (Spec §2.2.2.5.x).
///
/// The first `min(len, cap)` handles are written into `out_handles`,
/// `*out_count` is set to the **total** number of matched writers, and the
/// return value is `OutOfResources` when `len > cap` (so a caller retries with
/// a buffer of `*out_count`), `Ok` otherwise. A NULL `out_handles` with
/// `cap == 0` asks for the length alone (`docs/specs/zerodds-c-api-1.0.md`
/// §2.8).
///
/// # Safety
/// `dr`, `out_count` valid; `out_handles[0..cap]` writeable, or `out_handles`
/// NULL exactly when `cap == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_matched_publications(
    dr: *mut ZeroDdsDataReader,
    out_handles: *mut u64,
    out_count: *mut usize,
    cap: usize,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    if dr.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out_count NULL-checked above; the
    // buffer contract is enforced by `copy_list_out`.
    unsafe {
        let drr = &*dr;
        let handles: Vec<u64> = drr
            .rt
            .user_reader_matched_publication_handles(drr.eid)
            .iter()
            .map(|h| h.as_raw())
            .collect();
        copy_list_out(out_handles, cap, &handles, out_count)
    }
}

/// `get_matched_publication_data(handle)` — DDS 1.4 §2.2.2.5.3; as
/// [`zerodds_dw_get_matched_subscription_data`], for a writer this reader
/// is matched with. The caller releases `out` with
/// `zerodds_publication_builtin_topic_data_free`.
///
/// # Safety
/// `dr`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_matched_publication_data(
    dr: *mut ZeroDdsDataReader,
    handle: u64,
    out: *mut crate::builtin_ffi::ZeroDdsPublicationBuiltinTopicData,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr NULL-checked above.
    let drr = unsafe { &*dr };
    let Some(data) = drr.rt.user_reader_matched_publication_data(
        drr.eid,
        zerodds_dcps::instance_handle::InstanceHandle::from_raw(handle),
    ) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    write_publication_data(
        out,
        &zerodds_dcps::builtin_topics::PublicationBuiltinTopicData::from_wire(&data),
    )
}

/// Fills `out` with the C form of `s`; the caller releases it with
/// `zerodds_subscription_builtin_topic_data_free`.
fn write_subscription_data(
    out: *mut crate::builtin_ffi::ZeroDdsSubscriptionBuiltinTopicData,
    s: &zerodds_dcps::builtin_topics::SubscriptionBuiltinTopicData,
) -> c_int {
    let Some(data) = crate::builtin_ffi::subscription_to_c(s) else {
        // A name with a NUL in it has no C string; nothing is written.
        return ZeroDdsStatus::InvalidUtf8 as c_int;
    };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { *out = data };
    ZeroDdsStatus::Ok as c_int
}

/// The publication half of [`write_subscription_data`].
fn write_publication_data(
    out: *mut crate::builtin_ffi::ZeroDdsPublicationBuiltinTopicData,
    s: &zerodds_dcps::builtin_topics::PublicationBuiltinTopicData,
) -> c_int {
    let Some(data) = crate::builtin_ffi::publication_to_c(s) else {
        return ZeroDdsStatus::InvalidUtf8 as c_int;
    };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { *out = data };
    ZeroDdsStatus::Ok as c_int
}

/// Gives the reader the type of its samples, so a QueryCondition can read the fields its
/// expression names. `type_object` is the root `CompleteTypeObject`, `dependencies[0..count]`
/// (each `dependency_lens[i]` bytes) the complete TypeObjects it refers to, as
/// `topic_type_support<T>::type_object_dependency(i)` emits them.
///
/// `BadParameter` for a NULL argument or a TypeObject that is missing, malformed or does not
/// match `is_keyed` / `extensibility`.
///
/// # Safety
/// `dr` valid; `type_object` readable for `len` bytes; `dependencies` and `dependency_lens`
/// readable for `count` entries, each dependency for its length; or `count == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_set_type_objects(
    dr: *mut ZeroDdsDataReader,
    type_object: *const u8,
    len: usize,
    dependencies: *const *const u8,
    dependency_lens: *const usize,
    count: usize,
    is_keyed: c_int,
    extensibility: u32,
) -> c_int {
    if dr.is_null()
        || type_object.is_null()
        || len == 0
        || (count > 0 && (dependencies.is_null() || dependency_lens.is_null()))
    {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let Ok(extensibility) = u8::try_from(extensibility) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    // SAFETY: every pointer NULL-checked; lengths per the caller.
    unsafe {
        let drr = &*dr;
        let mut deps: Vec<Vec<u8>> = Vec::with_capacity(count);
        for i in 0..count {
            let p = *dependencies.add(i);
            let n = *dependency_lens.add(i);
            if p.is_null() || n == 0 {
                return ZeroDdsStatus::BadParameter as c_int;
            }
            deps.push(core::slice::from_raw_parts(p, n).to_vec());
        }
        if drr.topic.is_null() {
            return ZeroDdsStatus::BadParameter as c_int;
        }
        let type_name = (*drr.topic).type_name.clone();
        let root = core::slice::from_raw_parts(type_object, len);
        match zerodds_dcps::serialized_type::SerializedTypeSupport::from_type_objects(
            &type_name,
            root,
            &deps,
            is_keyed != 0,
            extensibility,
        ) {
            Ok(support) => match drr.query_type.lock() {
                Ok(mut slot) => {
                    *slot = Some(support);
                    ZeroDdsStatus::Ok as c_int
                }
                Err(_) => ZeroDdsStatus::Error as c_int,
            },
            Err(_) => ZeroDdsStatus::BadParameter as c_int,
        }
    }
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

    /// DDS 1.4 §2.2.2.1.1.1: `set_qos` with NULL is `*_QOS_DEFAULT`, "the
    /// current default QoS set in the Entity's factory" — here the
    /// publisher's for a writer and the participant's for a topic. It was
    /// the specification's default.
    #[test]
    fn set_qos_with_null_takes_the_current_default_of_the_factory() {
        let p = mk(86);
        // SAFETY: every handle comes from a create call on `p`; the QoS
        // structs live for the test.
        unsafe {
            let t = zerodds_dp_create_topic(p, c"NullQos".as_ptr(), c"NT".as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            assert!(!dw.is_null());

            let mut wqos: ZeroDdsDataWriterQos = core::mem::zeroed();
            assert_eq!(zerodds_pub_get_default_datawriter_qos(pubh, &mut wqos), 0);
            wqos.transport_priority.value = 0x11;
            assert_eq!(zerodds_pub_set_default_datawriter_qos(pubh, &wqos), 0);
            assert_eq!(zerodds_dw_set_qos(dw, ptr::null()), 0);
            let mut got: ZeroDdsDataWriterQos = core::mem::zeroed();
            assert_eq!(zerodds_dw_get_qos(dw, &mut got), 0);
            assert_eq!(got.transport_priority.value, 0x11);

            let mut tqos: ZeroDdsTopicQos = core::mem::zeroed();
            assert_eq!(zerodds_dp_get_default_topic_qos(p, &mut tqos), 0);
            tqos.transport_priority.value = 0x22;
            assert_eq!(zerodds_dp_set_default_topic_qos(p, &tqos), 0);
            assert_eq!(crate::topic_ffi::zerodds_topic_set_qos(t, ptr::null()), 0);
            let mut tgot: ZeroDdsTopicQos = core::mem::zeroed();
            assert_eq!(crate::topic_ffi::zerodds_topic_get_qos(t, &mut tgot), 0);
            assert_eq!(tgot.transport_priority.value, 0x22);
        }
        cleanup(p);
    }

    /// DDS 1.4 §2.3.2: a timestamp's nanosec is below 10^9. The three
    /// `*_w_timestamp` lifecycle calls refuse one out of range with
    /// BAD_PARAMETER and take one in range.
    #[test]
    fn dw_w_timestamp_calls_refuse_a_nanosec_out_of_range() {
        let p = mk(82);
        let n = c"TS";
        let tn = c"TST";
        let key = [0xCDu8; 16];
        let bad = ZeroDdsStatus::BadParameter as c_int;
        // SAFETY: p+n+tn+key live for the test; the extern fns document their pledges.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            let mut handle = 0u64;
            assert_eq!(
                zerodds_dw_register_instance_w_timestamp(
                    dw,
                    key.as_ptr(),
                    16,
                    1,
                    1_000_000_000,
                    &mut handle
                ),
                bad
            );
            assert_eq!(
                zerodds_dw_register_instance_w_timestamp(dw, key.as_ptr(), 16, 1, 5, &mut handle),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                zerodds_dw_dispose_w_timestamp(dw, key.as_ptr(), handle, 1, 1_000_000_000),
                bad
            );
            assert_eq!(
                zerodds_dw_unregister_instance_w_timestamp(dw, handle, 1, 1_000_000_000),
                bad
            );
            assert_eq!(
                zerodds_dw_dispose_w_timestamp(dw, key.as_ptr(), handle, 1, 5),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                zerodds_dw_unregister_instance_w_timestamp(dw, handle, 1, 6),
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
    fn full_key_registration_is_bounded_and_does_not_truncate() {
        let p = mk(77);
        // SAFETY: all handles and buffers are owned by this test.
        unsafe {
            let topic = zerodds_dp_create_topic(p, c"FullKey".as_ptr(), c"T".as_ptr(), ptr::null());
            let publisher = zerodds_dp_create_publisher(p, ptr::null());
            let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
            (*writer).qos.lock().unwrap().resource_limits.max_instances = 3;
            let a = [
                0, 0, 0, 6, b'a', b'b', b'c', b'd', b'X', 0, 0, 0, 0, 0, 0, 0,
            ];
            let mut b = a;
            b[8] = b'Y';
            let mut handles = Vec::new();
            for key in [a, b, [0; 16]] {
                let mut handle = 99;
                assert_eq!(
                    zerodds_dw_lookup_instance(writer, key.as_ptr(), 16, &mut handle),
                    ZeroDdsStatus::Ok as c_int
                );
                assert_eq!(handle, 0, "unseen key has no registered handle");
                assert_eq!(
                    zerodds_dw_register_instance(writer, key.as_ptr(), 16, &mut handle),
                    ZeroDdsStatus::Ok as c_int
                );
                assert_ne!(handle, 0);
                handles.push(handle);
                assert_eq!(
                    (*writer).instances.key_hash_for_handle(
                        zerodds_dcps::instance_handle::InstanceHandle::from_raw(handle)
                    ),
                    Some(key)
                );
                let mut repeated = 0;
                assert_eq!(
                    zerodds_dw_register_instance(writer, key.as_ptr(), 16, &mut repeated),
                    ZeroDdsStatus::Ok as c_int
                );
                assert_eq!(repeated, handle);
            }
            assert_ne!(handles[0], handles[1]);
            assert_ne!(handles[1], handles[2]);
            let mut rejected = 0;
            assert_eq!(
                zerodds_dw_register_instance(writer, [9; 16].as_ptr(), 16, &mut rejected),
                ZeroDdsStatus::OutOfResources as c_int
            );
            assert_eq!((*writer).instances.len(), 3);
            assert_eq!(
                zerodds_dw_unregister_instance(writer, 999),
                ZeroDdsStatus::BadHandle as c_int
            );
            assert_eq!(
                zerodds_dw_unregister_instance(writer, handles[0]),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                (*writer)
                    .instances
                    .get_by_handle(zerodds_dcps::instance_handle::InstanceHandle::from_raw(
                        handles[0]
                    ))
                    .unwrap()
                    .writer_count,
                0,
                "repeated registration must not accumulate writers"
            );
        }
        cleanup(p);
    }

    #[test]
    fn dw_get_key_value_never_reconstructs_a_holder_from_a_handle() {
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
                ZeroDdsStatus::BadHandle as c_int
            );
            let mut handle = 0;
            let key = [0xAB; 16];
            assert_eq!(
                zerodds_dw_register_instance(dw, key.as_ptr(), 16, &mut handle),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                zerodds_dw_get_key_value(dw, handle, buf.as_mut_ptr(), &mut len),
                ZeroDdsStatus::BadParameter as c_int
            );
        }
        assert_eq!(len, 4, "no invented holder length");
        assert_eq!(buf, [0; 4], "no invented holder bytes");
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

    /// A writer and a reader of one participant match each other, and
    /// each reads the other's data. The builtin DCPSSubscription reader
    /// holds discovered readers only: the handle was listed and its data
    /// answered NO_DATA. A handle that names no matched endpoint is
    /// BAD_PARAMETER (DDS 1.4 §2.2.2.4.2).
    #[test]
    fn matched_data_of_a_same_participant_pair() {
        let p = mk(83);
        let n = c"MatchedDataC";
        let tn = c"TT";
        let mut subs = [0u64; 4];
        let mut pubs = [0u64; 4];
        let mut n_subs = 0usize;
        let mut n_pubs = 0usize;
        // SAFETY: p+n+tn live for the test; every handle comes from a
        // create call on them.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert_eq!(
                zerodds_dw_get_matched_subscriptions(dw, subs.as_mut_ptr(), &mut n_subs, 4),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                zerodds_dr_get_matched_publications(dr, pubs.as_mut_ptr(), &mut n_pubs, 4),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!((n_subs, n_pubs), (1, 1));

            let mut sd =
                core::mem::zeroed::<crate::builtin_ffi::ZeroDdsSubscriptionBuiltinTopicData>();
            assert_eq!(
                zerodds_dw_get_matched_subscription_data(dw, subs[0], &mut sd),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(std::ffi::CStr::from_ptr(sd.topic_name), n);
            crate::builtin_ffi::zerodds_subscription_builtin_topic_data_free(&mut sd);

            let mut pd =
                core::mem::zeroed::<crate::builtin_ffi::ZeroDdsPublicationBuiltinTopicData>();
            assert_eq!(
                zerodds_dr_get_matched_publication_data(dr, pubs[0], &mut pd),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(std::ffi::CStr::from_ptr(pd.type_name), tn);
            crate::builtin_ffi::zerodds_publication_builtin_topic_data_free(&mut pd);

            assert_eq!(
                zerodds_dw_get_matched_subscription_data(dw, 12345, &mut sd),
                ZeroDdsStatus::BadParameter as c_int
            );
            assert_eq!(
                zerodds_dr_get_matched_publication_data(dr, subs[0], &mut pd),
                ZeroDdsStatus::BadParameter as c_int
            );
        }
        cleanup(p);
    }

    /// `cap == 0` with a NULL buffer asks for the total length; a NULL buffer
    /// with `cap > 0` is refused, for both matched listings
    /// (`docs/specs/zerodds-c-api-1.0.md` §2.8).
    #[test]
    fn matched_listings_length_query_and_null_contract() {
        // 76 is reserved (crates/test-domains); the topic names here match
        // no other test on it, so its lists stay empty.
        let p = mk(76);
        let mut count = 0usize;
        // SAFETY: every handle comes from a create call on `p`.
        unsafe {
            // A writer with no reader and a reader with no writer: each list
            // stays empty, so the length query is deterministic.
            let tw =
                zerodds_dp_create_topic(p, c"MatchedLenW".as_ptr(), c"TT".as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(pubh, tw, ptr::null());
            let tr =
                zerodds_dp_create_topic(p, c"MatchedLenR".as_ptr(), c"TT".as_ptr(), ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            let dr = zerodds_sub_create_datareader(sub, tr, ptr::null());

            let rc = zerodds_dw_get_matched_subscriptions(dw, ptr::null_mut(), &mut count, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 0);
            let rc = zerodds_dw_get_matched_subscriptions(dw, ptr::null_mut(), &mut count, 8);
            assert_eq!(rc, ZeroDdsStatus::BadParameter as c_int);

            let rc = zerodds_dr_get_matched_publications(dr, ptr::null_mut(), &mut count, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 0);
            let rc = zerodds_dr_get_matched_publications(dr, ptr::null_mut(), &mut count, 8);
            assert_eq!(rc, ZeroDdsStatus::BadParameter as c_int);
        }
        cleanup(p);
    }

    /// DDS 1.4 §2.2.3 through C: a writer's and a reader's QoS reach the
    /// runtime at the creation — LATENCY_BUDGET and TIME_BASED_FILTER were
    /// left at their defaults there — and a `set_qos` of a changeable
    /// policy is announced, TRANSPORT_PRIORITY included, while one of an
    /// immutable policy answers IMMUTABLE_POLICY. `set_qos` stored the QoS
    /// in the handle and stopped.
    #[test]
    fn c_qos_reaches_the_runtime_and_an_immutable_change_is_refused() {
        use crate::qos_ffi::ZeroDdsDuration;
        let p = mk(84);
        let ms = |millis: u32| ZeroDdsDuration {
            sec: 0,
            nanosec: millis * 1_000_000,
        };
        // SAFETY: every handle comes from a create call on `p`.
        unsafe {
            let t = zerodds_dp_create_topic(p, c"CQosReach".as_ptr(), c"TT".as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());

            let mut wqos: ZeroDdsDataWriterQos = core::mem::zeroed();
            assert_eq!(zerodds_pub_get_default_datawriter_qos(pubh, &mut wqos), 0);
            wqos.latency_budget.duration = ms(5);
            let dw = zerodds_pub_create_datawriter(pubh, t, &wqos);
            assert!(!dw.is_null());
            let dwr = &*dw;
            let announced = || {
                dwr.rt
                    .latest_announced_publication(dwr.eid)
                    .expect("announced")
            };
            assert_eq!(
                announced().latency_budget.duration,
                zerodds_qos::Duration::from_millis(5)
            );

            wqos.deadline.period = ms(300);
            wqos.transport_priority.value = 0x28;
            assert_eq!(zerodds_dw_set_qos(dw, &wqos), ZeroDdsStatus::Ok as c_int);
            assert_eq!(
                announced().deadline.period,
                zerodds_qos::Duration::from_millis(300)
            );
            assert_eq!(dwr.rt.user_writer_transport_priority(dwr.eid), 0x28);

            wqos.durability.kind = 1; // TRANSIENT_LOCAL
            assert_eq!(
                zerodds_dw_set_qos(dw, &wqos),
                ZeroDdsStatus::ImmutablePolicy as c_int
            );

            let mut rqos: ZeroDdsDataReaderQos = core::mem::zeroed();
            assert_eq!(zerodds_sub_get_default_datareader_qos(sub, &mut rqos), 0);
            rqos.time_based_filter.minimum_separation = ms(40);
            let dr = zerodds_sub_create_datareader(sub, t, &rqos);
            assert!(!dr.is_null());
            let drr = &*dr;
            assert_eq!(
                drr.rt
                    .latest_announced_subscription(drr.eid)
                    .expect("announced")
                    .time_based_filter
                    .minimum_separation,
                zerodds_qos::Duration::from_millis(40)
            );
            rqos.time_based_filter.minimum_separation = ms(80);
            assert_eq!(zerodds_dr_set_qos(dr, &rqos), ZeroDdsStatus::Ok as c_int);
            assert_eq!(
                drr.rt
                    .latest_announced_subscription(drr.eid)
                    .expect("announced")
                    .time_based_filter
                    .minimum_separation,
                zerodds_qos::Duration::from_millis(80)
            );
        }
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

    /// QB-cluster regression: the C-FFI create path MUST plumb the
    /// PARTITION names from the writer/reader QoS onto the runtime endpoint
    /// config so `partitions_overlap` (DDS 1.4 §2.2.3.10) gates intra-runtime
    /// matching. A writer in partition ["A"] and a reader in ["B"] must NOT
    /// exchange data; a reader in ["A"] receives it. Before the fix the
    /// create path hardcoded an empty partition → every endpoint landed in
    /// the default partition and the mismatched reader wrongly received data.
    #[test]
    fn partition_isolation_over_ffi_create_path() {
        use super::{
            zerodds_pub_get_default_datawriter_qos, zerodds_sub_get_default_datareader_qos,
        };
        use crate::publisher_ffi::zerodds_dw_write;
        use crate::qos_ffi::{ZeroDdsDataReaderQos, ZeroDdsDataWriterQos};
        use crate::subscriber_ffi::{ZeroDdsSampleArray, zerodds_dr_return_loan, zerodds_dr_take};
        use std::ffi::CString;

        let p = mk(78);
        let n = c"PartTopic";
        let tn = c"RawBytes";
        // Caller-owned C-string array for the partition name lists. Must
        // outlive the create calls (cstr_vec copies during decode, but keep
        // them alive for the whole block to be safe).
        let part_a = CString::new("A").unwrap();
        let part_b = CString::new("B").unwrap();
        let a_arr: [*const core::ffi::c_char; 1] = [part_a.as_ptr()];
        let b_arr: [*const core::ffi::c_char; 1] = [part_b.as_ptr()];

        // SAFETY: p+topic+CStrings live for the whole block; out-pointers on
        // stack-local slots; the extern fns document their pledges.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());

            // Writer in partition ["A"].
            let mut wqos: ZeroDdsDataWriterQos = core::mem::zeroed();
            assert_eq!(
                zerodds_pub_get_default_datawriter_qos(pubh, &mut wqos),
                ZeroDdsStatus::Ok as c_int
            );
            wqos.partition.names = a_arr.as_ptr();
            wqos.partition.names_len = 1;
            let dw = zerodds_pub_create_datawriter(pubh, t, &wqos);
            assert!(!dw.is_null());

            // Reader 1 in partition ["B"] — must NOT match.
            let mut rqos_b: ZeroDdsDataReaderQos = core::mem::zeroed();
            assert_eq!(
                zerodds_sub_get_default_datareader_qos(sub, &mut rqos_b),
                ZeroDdsStatus::Ok as c_int
            );
            rqos_b.partition.names = b_arr.as_ptr();
            rqos_b.partition.names_len = 1;
            let dr_b = zerodds_sub_create_datareader(sub, t, &rqos_b);
            assert!(!dr_b.is_null());

            // Reader 2 in partition ["A"] — must match.
            let mut rqos_a: ZeroDdsDataReaderQos = core::mem::zeroed();
            assert_eq!(
                zerodds_sub_get_default_datareader_qos(sub, &mut rqos_a),
                ZeroDdsStatus::Ok as c_int
            );
            rqos_a.partition.names = a_arr.as_ptr();
            rqos_a.partition.names_len = 1;
            let dr_a = zerodds_sub_create_datareader(sub, t, &rqos_a);
            assert!(!dr_a.is_null());

            // Write a few samples; intra-runtime dispatch is synchronous via
            // the reader mpsc channel.
            let payload = [0x11u8, 0x22, 0x33, 0x44];
            for _ in 0..3 {
                assert_eq!(
                    zerodds_dw_write(dw, payload.as_ptr(), payload.len(), 0),
                    ZeroDdsStatus::Ok as c_int
                );
            }

            // Give the dispatch a brief, bounded window. Mismatched reader
            // must stay empty; matching reader must receive.
            let mut got_a = 0usize;
            let mut got_b = 0usize;
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
            while std::time::Instant::now() < deadline && got_a == 0 {
                let mut arr_a: ZeroDdsSampleArray = core::mem::zeroed();
                let mut arr_b: ZeroDdsSampleArray = core::mem::zeroed();
                let _ = zerodds_dr_take(dr_a, &mut arr_a, 10, 0, 0, 0);
                let _ = zerodds_dr_take(dr_b, &mut arr_b, 10, 0, 0, 0);
                got_a += arr_a.count;
                got_b += arr_b.count;
                zerodds_dr_return_loan(dr_a, &mut arr_a);
                zerodds_dr_return_loan(dr_b, &mut arr_b);
                if got_a == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }

            assert!(
                got_a >= 1,
                "reader in matching partition [A] must receive data, got {got_a}"
            );
            assert_eq!(
                got_b, 0,
                "reader in mismatched partition [B] must receive NOTHING, got {got_b}"
            );
            // dr_a/dr_b/dw are tracked by the participant and freed by
            // cleanup() → delete_contained_entities; do NOT free here.
        }
        cleanup(p);
    }

    /// QB-rest regression (#79, issue 1): EXCLUSIVE ownership on the C-FFI
    /// SAME-RUNTIME write→take path. Two real DataWriters (no channel
    /// injection) on the same instance — a weak one (strength 1) and a strong
    /// one (strength 10) — publish via `zerodds_dw_write`; intra-runtime
    /// dispatch threads each writer's `ownership_strength` into the delivered
    /// sample (dcps `intra_runtime_dispatch_alive`), and the EXCLUSIVE reader's
    /// take path arbitrates through the validated
    /// `InstanceTracker::should_accept_sample_under_exclusive_ownership`
    /// (DDS 1.4 §2.2.3.23). The strong writer publishes first and owns the
    /// instance; the weak writer's samples on that instance are suppressed, so
    /// the reader sees ONLY the strength-10 writer. This closes the cpp/python/
    /// c# gap (their FFI take path now arbitrates same-runtime samples).
    #[test]
    fn exclusive_ownership_same_runtime_ffi_suppresses_weaker_writer() {
        use crate::publisher_ffi::zerodds_dw_write;
        use crate::qos_ffi::{ZeroDdsDataReaderQos, ZeroDdsDataWriterQos};
        use crate::subscriber_ffi::{ZeroDdsSampleArray, zerodds_dr_return_loan, zerodds_dr_take};

        let p = mk(80);
        let n = c"OwnTopic";
        let tn = c"RawBytes";
        // SAFETY: p+topic live for the whole block; out-pointers stack-local.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());

            // Strong writer: EXCLUSIVE, strength 10.
            let mut wqos_strong: ZeroDdsDataWriterQos = core::mem::zeroed();
            wqos_strong.ownership.kind = 1; // Exclusive
            wqos_strong.ownership_strength.value = 10;
            let dw_strong = zerodds_pub_create_datawriter(pubh, t, &wqos_strong);
            assert!(!dw_strong.is_null());

            // Weak writer: EXCLUSIVE, strength 1, same topic+instance.
            let mut wqos_weak: ZeroDdsDataWriterQos = core::mem::zeroed();
            wqos_weak.ownership.kind = 1; // Exclusive
            wqos_weak.ownership_strength.value = 1;
            let dw_weak = zerodds_pub_create_datawriter(pubh, t, &wqos_weak);
            assert!(!dw_weak.is_null());

            // EXCLUSIVE reader.
            let mut rqos: ZeroDdsDataReaderQos = core::mem::zeroed();
            rqos.ownership.kind = 1; // Exclusive
            let dr = zerodds_sub_create_datareader(sub, t, &rqos);
            assert!(!dr.is_null());

            // Same instance payload (identical bytes → identical key hash).
            let payload = [0xAAu8, 0xBB, 0xCC, 0xDD];
            // Strong writer publishes first and becomes the instance owner.
            for _ in 0..3 {
                assert_eq!(
                    zerodds_dw_write(dw_strong, payload.as_ptr(), payload.len(), 0),
                    ZeroDdsStatus::Ok as c_int
                );
            }
            // Weak writer publishes the same instance — must be suppressed.
            for _ in 0..3 {
                assert_eq!(
                    zerodds_dw_write(dw_weak, payload.as_ptr(), payload.len(), 0),
                    ZeroDdsStatus::Ok as c_int
                );
            }

            // The strong writer's GUID-derived publication handle: the reader
            // may only ever surface samples carrying THIS handle. The GUID is
            // (runtime guid_prefix + writer eid), exactly as the intra-runtime
            // dispatch stamps it (dcps `intra_runtime_dispatch_alive`).
            let strong_guid = {
                let dwr = &*dw_strong;
                zerodds_rtps::wire_types::Guid::new(dwr.rt.guid_prefix, dwr.eid).to_bytes()
            };
            let strong_handle = crate::subscriber_ffi::u64_from_guid(strong_guid);

            let mut total = 0usize;
            let mut handles: Vec<u64> = Vec::new();
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
            while std::time::Instant::now() < deadline {
                let mut arr: ZeroDdsSampleArray = core::mem::zeroed();
                let _ = zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0);
                for i in 0..arr.count {
                    handles.push((*arr.infos.add(i)).publication_handle);
                    total += 1;
                }
                zerodds_dr_return_loan(dr, &mut arr);
                if total >= 3 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(
                total >= 1,
                "strong writer's samples must reach the reader, got {total}"
            );
            assert!(
                handles.iter().all(|&h| h == strong_handle),
                "EXCLUSIVE reader must only see the strength-10 writer; \
                 saw publication handles {handles:?}, expected only {strong_handle}"
            );
        }
        cleanup(p);
    }

    /// QB-rest regression (#79, issue 2): PARTITION OUT-marshalling. A writer/
    /// reader QoS with a non-empty partition list, set via `set_qos`, must be
    /// reflected back out by `get_qos` (previously `dw_qos_to_c`/`dr_qos_to_c`
    /// hardcoded `names_len = 0`). The names round-trip through the entity-owned
    /// `PartitionOutCache` (DDS 1.4 §2.2.3.10).
    #[test]
    fn partition_round_trips_out_via_get_qos() {
        use crate::qos_ffi::{ZeroDdsDataReaderQos, ZeroDdsDataWriterQos};
        use std::ffi::{CStr, CString};

        let p = mk(81);
        let n = c"PartOutTopic";
        let tn = c"RawBytes";
        let part_x = CString::new("X").unwrap();
        let part_y = CString::new("Y").unwrap();
        let arr_in: [*const core::ffi::c_char; 2] = [part_x.as_ptr(), part_y.as_ptr()];

        // SAFETY: p+topic+CStrings live for the whole block.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());

            // ---- Writer ----
            let mut wqos: ZeroDdsDataWriterQos = core::mem::zeroed();
            wqos.partition.names = arr_in.as_ptr();
            wqos.partition.names_len = 2;
            let dw = zerodds_pub_create_datawriter(pubh, t, &wqos);
            assert!(!dw.is_null());
            assert_eq!(zerodds_dw_set_qos(dw, &wqos), ZeroDdsStatus::Ok as c_int);

            let mut wqos_out: ZeroDdsDataWriterQos = core::mem::zeroed();
            assert_eq!(
                zerodds_dw_get_qos(dw, &mut wqos_out),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                wqos_out.partition.names_len, 2,
                "writer partition must round-trip out"
            );
            let got: Vec<String> = (0..wqos_out.partition.names_len)
                .map(|i| {
                    let cp = *wqos_out.partition.names.add(i);
                    CStr::from_ptr(cp).to_string_lossy().into_owned()
                })
                .collect();
            assert_eq!(got, alloc::vec!["X".to_string(), "Y".to_string()]);

            // ---- Reader ----
            let mut rqos: ZeroDdsDataReaderQos = core::mem::zeroed();
            rqos.partition.names = arr_in.as_ptr();
            rqos.partition.names_len = 2;
            let dr = zerodds_sub_create_datareader(sub, t, &rqos);
            assert!(!dr.is_null());
            assert_eq!(zerodds_dr_set_qos(dr, &rqos), ZeroDdsStatus::Ok as c_int);

            let mut rqos_out: ZeroDdsDataReaderQos = core::mem::zeroed();
            assert_eq!(
                zerodds_dr_get_qos(dr, &mut rqos_out),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                rqos_out.partition.names_len, 2,
                "reader partition must round-trip out"
            );
            let got_r: Vec<String> = (0..rqos_out.partition.names_len)
                .map(|i| {
                    let cp = *rqos_out.partition.names.add(i);
                    CStr::from_ptr(cp).to_string_lossy().into_owned()
                })
                .collect();
            assert_eq!(got_r, alloc::vec!["X".to_string(), "Y".to_string()]);

            // Empty partition → names_len 0 (no stale pointers).
            let mut wqos_empty: ZeroDdsDataWriterQos = core::mem::zeroed();
            assert_eq!(
                zerodds_dw_set_qos(dw, &wqos_empty),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                zerodds_dw_get_qos(dw, &mut wqos_empty),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(wqos_empty.partition.names_len, 0);
        }
        cleanup(p);
    }

    /// `zerodds_dw_get_key_value` reports the needed length in `*inout_len` and
    /// returns `OutOfResources` when the buffer is too small, so a stored key
    /// holder is not truncated silently. An instance known by its hash alone,
    /// which the type does not spell out, has nothing to return and is refused
    /// with `BadParameter`
    /// (`docs/specs/zerodds-c-api-1.0.md` §2.8 for the list calls; this byte
    /// buffer reports the needed length the same way).
    #[test]
    fn dw_get_key_value_reports_the_needed_length() {
        // A leased domain: a fixed one in the lease band can be handed to
        // another test at the same time (crates/test-domains).
        let lease = zerodds_test_domains::lease_domain();
        let p = mk(lease.id() as u32);
        let holder = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        // SAFETY: every handle comes from a create call on `p`; the buffer and
        // length are stack-local.
        unsafe {
            let t = zerodds_dp_create_topic(p, c"GKVT".as_ptr(), c"TT".as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());

            let handle = (*dw)
                .instances
                .register([0x42u8; 16], holder.to_vec(), None)
                .as_raw();

            // Empty result: a holder of no bytes has nothing to copy.
            let empty_handle = (*dw)
                .instances
                .register([0x43u8; 16], Vec::new(), None)
                .as_raw();
            let mut len = 0usize;
            let mut buf = [0u8; 8];
            assert_eq!(
                zerodds_dw_get_key_value(dw, empty_handle, buf.as_mut_ptr(), &mut len),
                ZeroDdsStatus::BadParameter as c_int
            );

            // One byte more than the buffer holds.
            let mut len = holder.len() - 1;
            assert_eq!(
                zerodds_dw_get_key_value(dw, handle, buf.as_mut_ptr(), &mut len),
                ZeroDdsStatus::OutOfResources as c_int
            );
            assert_eq!(len, holder.len(), "the needed length is reported");
            assert_eq!(buf, [0u8; 8], "nothing is copied when it does not fit");

            // Exactly full.
            let mut len = holder.len();
            assert_eq!(
                zerodds_dw_get_key_value(dw, handle, buf.as_mut_ptr(), &mut len),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(len, holder.len());
            assert_eq!(&buf[..holder.len()], &holder[..]);

            // An empty buffer asks for the needed length.
            let mut len = 0usize;
            assert_eq!(
                zerodds_dw_get_key_value(dw, handle, buf.as_mut_ptr(), &mut len),
                ZeroDdsStatus::OutOfResources as c_int
            );
            assert_eq!(len, holder.len());
        }
        cleanup(p);
    }

    /// [`zerodds_dw_get_key_value`] boundary cases for a DataReader's stored
    /// key holder.
    #[test]
    fn dr_get_key_value_reports_the_needed_length() {
        // A leased domain: a fixed one in the lease band can be handed to
        // another test at the same time (crates/test-domains).
        let lease = zerodds_test_domains::lease_domain();
        let p = mk(lease.id() as u32);
        let holder = [0x01u8, 0x02, 0x03, 0x04, 0x05];
        // SAFETY: every handle comes from a create call on `p`; the buffer and
        // length are stack-local.
        unsafe {
            let t = zerodds_dp_create_topic(p, c"GKVR".as_ptr(), c"TT".as_ptr(), ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());

            let handle = (*dr)
                .instances
                .register([0x44u8; 16], holder.to_vec(), None)
                .as_raw();

            let empty_handle = (*dr)
                .instances
                .register([0x45u8; 16], Vec::new(), None)
                .as_raw();
            let mut len = 0usize;
            let mut buf = [0u8; 5];
            assert_eq!(
                zerodds_dr_get_key_value(dr, empty_handle, buf.as_mut_ptr(), &mut len),
                ZeroDdsStatus::BadParameter as c_int
            );

            let mut len = holder.len() - 1;
            assert_eq!(
                zerodds_dr_get_key_value(dr, handle, buf.as_mut_ptr(), &mut len),
                ZeroDdsStatus::OutOfResources as c_int
            );
            assert_eq!(len, holder.len(), "the needed length is reported");
            assert_eq!(buf, [0u8; 5], "nothing is copied when it does not fit");

            let mut len = holder.len();
            assert_eq!(
                zerodds_dr_get_key_value(dr, handle, buf.as_mut_ptr(), &mut len),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(&buf[..holder.len()], &holder[..]);

            let mut len = 0usize;
            assert_eq!(
                zerodds_dr_get_key_value(dr, handle, buf.as_mut_ptr(), &mut len),
                ZeroDdsStatus::OutOfResources as c_int
            );
            assert_eq!(len, holder.len());
        }
        cleanup(p);
    }
}
