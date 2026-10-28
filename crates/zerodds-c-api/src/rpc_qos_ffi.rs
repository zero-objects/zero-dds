// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The QoS side of the DDS-RPC 1.0 C++ binding (§7.10).
//!
//! * `zerodds_endpoint_qos_from_profile_url` (feature `rpc`, in
//!   `rpc_profile_ffi`) resolves a `@Qos` profile URL
//!   (§7.10.1) to the endpoint QoS an XML QoS profile names.
//! * [`zerodds_writer_set_qos`] and [`zerodds_reader_set_qos`] change the
//!   mutable policies of a live runtime endpoint (§7.10.2: "The users may
//!   modify the mutable QoS policies at run-time").

use core::ffi::c_int;

use zerodds_dcps::runtime::{ReaderQosUpdate, WriterQosUpdate};
use zerodds_qos::{DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, ReliabilityKind};

use crate::{ZeroDdsEndpointQos, ZeroDdsReader, ZeroDdsStatus, ZeroDdsWriter, qos_duration};

#[cfg(feature = "rpc")]
pub use crate::rpc_profile_ffi::zerodds_endpoint_qos_from_profile_url;

/// Changes the mutable policies of a runtime writer: DEADLINE and LIFESPAN of
/// `qos`. Of the policies the DDS-RPC QoS carries (§7.10.2: reliability,
/// durability, history, resource limits, lifespan, deadline) DDS 1.4 §2.2.3
/// makes exactly these two changeable after the entity is enabled; the
/// others — RELIABILITY (with its `max_blocking_time`), DURABILITY, HISTORY
/// and RESOURCE_LIMITS — are "Changeable: No". A value of one of them that
/// differs from the writer's is `ImmutablePolicy`, and nothing is changed.
///
/// # Safety
/// `writer` from `zerodds_writer_create*`; `qos` points at a
/// [`ZeroDdsEndpointQos`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_set_qos(
    writer: *mut ZeroDdsWriter,
    qos: *const ZeroDdsEndpointQos,
) -> c_int {
    if writer.is_null() || qos.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL-checked above; valid per the # Safety doc.
    let (rt, eid, qos) = unsafe { ((*writer).rt.clone(), (*writer).eid, *qos) };
    let Some(announced) = rt.latest_announced_publication(eid) else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    if (announced.reliability.kind == ReliabilityKind::Reliable) != (qos.reliable != 0)
        || (announced.durability == DurabilityKind::TransientLocal) != (qos.transient_local != 0)
    {
        return ZeroDdsStatus::ImmutablePolicy as c_int;
    }
    let Ok(wanted) = crate::endpoint_qos(&qos) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    let Some((history, limits, blocking)) = rt.user_writer_history_and_limits(eid) else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    let wanted_blocking = (!wanted.max_blocking_time.is_infinite()).then(|| {
        core::time::Duration::from_nanos(
            u64::try_from(wanted.max_blocking_time.to_nanos()).unwrap_or(u64::MAX),
        )
    });
    if history != wanted.keep_last.unwrap_or(usize::MAX)
        || limits != wanted.resource_limits
        || blocking != wanted_blocking
    {
        return ZeroDdsStatus::ImmutablePolicy as c_int;
    }
    let Some(mut update) = rt.user_writer_qos(eid) else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    update.deadline = DeadlineQosPolicy {
        period: qos_duration(qos.deadline_ns),
    };
    update.lifespan = LifespanQosPolicy {
        duration: qos_duration(qos.lifespan_ns),
    };
    update_writer(&rt, eid, &update)
}

fn update_writer(
    rt: &zerodds_dcps::runtime::DcpsRuntime,
    eid: zerodds_rtps::wire_types::EntityId,
    update: &WriterQosUpdate,
) -> c_int {
    match rt.update_user_writer_qos(eid, update) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(zerodds_dcps::DdsError::BadParameter { .. }) => ZeroDdsStatus::BadParameter as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Changes the mutable policies of a runtime reader: DEADLINE of `qos`. Of
/// the policies the DDS-RPC QoS carries DDS 1.4 §2.2.3 makes only DEADLINE
/// changeable on a reader (LIFESPAN is a writer policy); RELIABILITY,
/// DURABILITY, HISTORY and RESOURCE_LIMITS are "Changeable: No". A differing
/// value of one of them is `ImmutablePolicy` and nothing is changed.
///
/// # Safety
/// `reader` from `zerodds_reader_create*`; `qos` points at a
/// [`ZeroDdsEndpointQos`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_set_qos(
    reader: *mut ZeroDdsReader,
    qos: *const ZeroDdsEndpointQos,
) -> c_int {
    if reader.is_null() || qos.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL-checked above; valid per the # Safety doc.
    let (rt, eid, qos) = unsafe { ((*reader).rt.clone(), (*reader).eid, *qos) };
    let Some(announced) = rt.latest_announced_subscription(eid) else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    if (announced.reliability.kind == ReliabilityKind::Reliable) != (qos.reliable != 0)
        || (announced.durability == DurabilityKind::TransientLocal) != (qos.transient_local != 0)
    {
        return ZeroDdsStatus::ImmutablePolicy as c_int;
    }
    let Ok(wanted) = crate::endpoint_qos(&qos) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    let Some(limits) = rt.user_reader_resource_limits(eid) else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    // SAFETY: NULL-checked above.
    let history = unsafe { (*reader).history_depth() };
    if limits != wanted.resource_limits || history != wanted.keep_last {
        return ZeroDdsStatus::ImmutablePolicy as c_int;
    }
    let Some(mut update) = rt.user_reader_qos(eid) else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    update.deadline = DeadlineQosPolicy {
        period: qos_duration(qos.deadline_ns),
    };
    update_reader(&rt, eid, &update)
}

fn update_reader(
    rt: &zerodds_dcps::runtime::DcpsRuntime,
    eid: zerodds_rtps::wire_types::EntityId,
    update: &ReaderQosUpdate,
) -> c_int {
    match rt.update_user_reader_qos(eid, update) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(zerodds_dcps::DdsError::BadParameter { .. }) => ZeroDdsStatus::BadParameter as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Sets the CDR form the samples of a runtime writer have: `0` is XCDR1
/// (classic CDR, `CDR_LE` in the encapsulation header), `2` is XCDR2. The
/// body the caller hands `zerodds_writer_write*` has to be in that form;
/// the encapsulation identifier names it. DDS-RPC 1.0 §7.5.1.1: the Basic
/// Service Mapping is for DDS 1.3 implementations, that is classic CDR.
///
/// `BadParameter` for another value. `PreconditionNotMet` when the writer's
/// representation is owned by a typed writer.
///
/// # Safety
/// `writer` from `zerodds_writer_create*`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_set_data_representation(
    writer: *mut ZeroDdsWriter,
    representation: c_int,
) -> c_int {
    if writer.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let id = match representation {
        0 => zerodds_rtps::publication_data::data_representation::XCDR,
        2 => zerodds_rtps::publication_data::data_representation::XCDR2,
        _ => return ZeroDdsStatus::BadParameter as c_int,
    };
    // SAFETY: NULL-checked above.
    let (rt, eid) = unsafe { ((*writer).rt.clone(), (*writer).eid) };
    match rt.set_user_writer_data_rep_override(eid, Some(alloc::vec![id])) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(zerodds_dcps::DdsError::BadParameter { .. }) => ZeroDdsStatus::BadHandle as c_int,
        Err(_) => ZeroDdsStatus::PreconditionNotMet as c_int,
    }
}
