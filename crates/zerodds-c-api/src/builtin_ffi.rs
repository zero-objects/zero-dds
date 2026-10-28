// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Built-in Topic Data + Discovered Endpoints C-FFI (Spec §2.2.5).
//!
//! Provides the 4 normative builtin topics:
//! - `DCPSParticipant` (§2.2.5.1) → `ZeroDdsParticipantBuiltinTopicData`
//! - `DCPSTopic` (§2.2.5.2) → `ZeroDdsTopicBuiltinTopicData`
//! - `DCPSPublication` (§2.2.5.3) → `ZeroDdsPublicationBuiltinTopicData`
//! - `DCPSSubscription` (§2.2.5.4) → `ZeroDdsSubscriptionBuiltinTopicData`

use alloc::ffi::CString;
use alloc::vec::Vec;
use core::ffi::{c_char, c_int};
use core::ptr;
use core::slice;

use zerodds_dcps::instance_handle::InstanceHandle;

use crate::ZeroDdsStatus;
use crate::entities::ZeroDdsDomainParticipant;
use crate::ffi_helpers::copy_list_out;
use crate::qos_ffi::{
    ZeroDdsDeadlineQosPolicy, ZeroDdsDestinationOrderQosPolicy, ZeroDdsDurabilityQosPolicy,
    ZeroDdsDurabilityServiceQosPolicy, ZeroDdsGroupDataQosPolicy, ZeroDdsHistoryQosPolicy,
    ZeroDdsLatencyBudgetQosPolicy, ZeroDdsLifespanQosPolicy, ZeroDdsLivelinessQosPolicy,
    ZeroDdsOwnershipQosPolicy, ZeroDdsOwnershipStrengthQosPolicy, ZeroDdsPartitionQosPolicy,
    ZeroDdsPresentationQosPolicy, ZeroDdsReliabilityQosPolicy, ZeroDdsResourceLimitsQosPolicy,
    ZeroDdsTimeBasedFilterQosPolicy, ZeroDdsTopicDataQosPolicy, ZeroDdsTransportPriorityQosPolicy,
    ZeroDdsUserDataQosPolicy,
};

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// `DCPSParticipant` sample (Spec §2.2.5.1).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsParticipantBuiltinTopicData {
    /// 16-byte GUID.
    pub guid: [u8; 16],
    /// Pointer to UserData bytes (static over the `dp` lifetime in the RC1
    /// snapshot — the caller copies if it outlives the data).
    pub user_data: *const u8,
    /// UserData length.
    pub user_data_len: usize,
}

/// `DCPSTopic` sample (Spec §2.2.5.2), with every member DDS 1.4
/// §2.2.5 lists for the TopicBuiltinTopicData.
///
/// The two names and the octet sequence are allocated by the call that
/// fills the struct and belong to the caller until it hands the struct
/// to [`zerodds_topic_builtin_topic_data_free`]; the policies carry no
/// pointers and need no release of their own.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsTopicBuiltinTopicData {
    /// Synthetic topic key (16 byte).
    pub key: [u8; 16],
    /// Topic name.
    pub name: *mut c_char,
    /// Type name.
    pub type_name: *mut c_char,
    /// DURABILITY.
    pub durability: ZeroDdsDurabilityQosPolicy,
    /// DURABILITY_SERVICE.
    pub durability_service: ZeroDdsDurabilityServiceQosPolicy,
    /// DEADLINE.
    pub deadline: ZeroDdsDeadlineQosPolicy,
    /// LATENCY_BUDGET.
    pub latency_budget: ZeroDdsLatencyBudgetQosPolicy,
    /// LIVELINESS.
    pub liveliness: ZeroDdsLivelinessQosPolicy,
    /// RELIABILITY.
    pub reliability: ZeroDdsReliabilityQosPolicy,
    /// TRANSPORT_PRIORITY.
    pub transport_priority: ZeroDdsTransportPriorityQosPolicy,
    /// LIFESPAN.
    pub lifespan: ZeroDdsLifespanQosPolicy,
    /// DESTINATION_ORDER.
    pub destination_order: ZeroDdsDestinationOrderQosPolicy,
    /// HISTORY.
    pub history: ZeroDdsHistoryQosPolicy,
    /// RESOURCE_LIMITS.
    pub resource_limits: ZeroDdsResourceLimitsQosPolicy,
    /// OWNERSHIP.
    pub ownership: ZeroDdsOwnershipQosPolicy,
    /// TOPIC_DATA.
    pub topic_data: ZeroDdsTopicDataQosPolicy,
}

/// `DCPSPublication` sample (Spec §2.2.5.3), with every member DDS 1.4
/// §2.2.5 lists for the PublicationBuiltinTopicData.
///
/// The strings, the octet sequences and the partition names are allocated
/// by the call that fills the struct and belong to the caller until it
/// hands the struct to [`zerodds_publication_builtin_topic_data_free`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPublicationBuiltinTopicData {
    /// Endpoint GUID (writer).
    pub key: [u8; 16],
    /// Owning participant GUID.
    pub participant_key: [u8; 16],
    /// Topic name.
    pub topic_name: *mut c_char,
    /// Type name.
    pub type_name: *mut c_char,
    /// DURABILITY.
    pub durability: ZeroDdsDurabilityQosPolicy,
    /// DURABILITY_SERVICE.
    pub durability_service: ZeroDdsDurabilityServiceQosPolicy,
    /// DEADLINE.
    pub deadline: ZeroDdsDeadlineQosPolicy,
    /// LATENCY_BUDGET.
    pub latency_budget: ZeroDdsLatencyBudgetQosPolicy,
    /// LIVELINESS.
    pub liveliness: ZeroDdsLivelinessQosPolicy,
    /// RELIABILITY.
    pub reliability: ZeroDdsReliabilityQosPolicy,
    /// LIFESPAN.
    pub lifespan: ZeroDdsLifespanQosPolicy,
    /// USER_DATA.
    pub user_data: ZeroDdsUserDataQosPolicy,
    /// OWNERSHIP.
    pub ownership: ZeroDdsOwnershipQosPolicy,
    /// OWNERSHIP_STRENGTH.
    pub ownership_strength: ZeroDdsOwnershipStrengthQosPolicy,
    /// DESTINATION_ORDER.
    pub destination_order: ZeroDdsDestinationOrderQosPolicy,
    /// PRESENTATION.
    pub presentation: ZeroDdsPresentationQosPolicy,
    /// PARTITION.
    pub partition: ZeroDdsPartitionQosPolicy,
    /// TOPIC_DATA.
    pub topic_data: ZeroDdsTopicDataQosPolicy,
    /// GROUP_DATA.
    pub group_data: ZeroDdsGroupDataQosPolicy,
}

/// `DCPSSubscription` sample (Spec §2.2.5.4), with every member DDS 1.4
/// §2.2.5 lists for the SubscriptionBuiltinTopicData. Ownership of the
/// allocated members as for [`ZeroDdsPublicationBuiltinTopicData`]; the
/// release is [`zerodds_subscription_builtin_topic_data_free`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsSubscriptionBuiltinTopicData {
    /// Endpoint GUID (reader).
    pub key: [u8; 16],
    /// Owning participant GUID.
    pub participant_key: [u8; 16],
    /// Topic name.
    pub topic_name: *mut c_char,
    /// Type name.
    pub type_name: *mut c_char,
    /// DURABILITY.
    pub durability: ZeroDdsDurabilityQosPolicy,
    /// DEADLINE.
    pub deadline: ZeroDdsDeadlineQosPolicy,
    /// LATENCY_BUDGET.
    pub latency_budget: ZeroDdsLatencyBudgetQosPolicy,
    /// LIVELINESS.
    pub liveliness: ZeroDdsLivelinessQosPolicy,
    /// RELIABILITY.
    pub reliability: ZeroDdsReliabilityQosPolicy,
    /// OWNERSHIP.
    pub ownership: ZeroDdsOwnershipQosPolicy,
    /// DESTINATION_ORDER.
    pub destination_order: ZeroDdsDestinationOrderQosPolicy,
    /// USER_DATA.
    pub user_data: ZeroDdsUserDataQosPolicy,
    /// TIME_BASED_FILTER.
    pub time_based_filter: ZeroDdsTimeBasedFilterQosPolicy,
    /// PRESENTATION.
    pub presentation: ZeroDdsPresentationQosPolicy,
    /// PARTITION.
    pub partition: ZeroDdsPartitionQosPolicy,
    /// TOPIC_DATA.
    pub topic_data: ZeroDdsTopicDataQosPolicy,
    /// GROUP_DATA.
    pub group_data: ZeroDdsGroupDataQosPolicy,
}

/// A C string of `s`, or `None` when `s` holds a NUL that a C string
/// cannot carry.
fn c_string(s: &str) -> Option<*mut c_char> {
    CString::new(s).ok().map(CString::into_raw)
}

/// An allocated copy of `bytes`; null for an empty sequence.
fn leak_octets(bytes: &[u8]) -> ZeroDdsUserDataQosPolicy {
    if bytes.is_empty() {
        return ZeroDdsUserDataQosPolicy {
            value: ptr::null(),
            value_len: 0,
        };
    }
    let boxed: alloc::boxed::Box<[u8]> = bytes.into();
    let value_len = boxed.len();
    ZeroDdsUserDataQosPolicy {
        value: alloc::boxed::Box::leak(boxed).as_ptr(),
        value_len,
    }
}

/// # Safety
/// `octets` came from [`leak_octets`] and is not released twice.
unsafe fn free_octets(octets: &mut ZeroDdsUserDataQosPolicy) {
    if !octets.value.is_null() && octets.value_len > 0 {
        // SAFETY: value/value_len are the Box<[u8]> leak_octets leaked.
        drop(unsafe {
            alloc::boxed::Box::from_raw(slice::from_raw_parts_mut(
                octets.value.cast_mut(),
                octets.value_len,
            ))
        });
    }
    octets.value = ptr::null();
    octets.value_len = 0;
}

/// Allocated C strings of `names` and the array that points at them, or
/// `None` when a name holds a NUL.
fn leak_partition(names: &[String]) -> Option<ZeroDdsPartitionQosPolicy> {
    if names.is_empty() {
        return Some(ZeroDdsPartitionQosPolicy {
            names: ptr::null(),
            names_len: 0,
        });
    }
    let mut ptrs: Vec<*const c_char> = Vec::with_capacity(names.len());
    for name in names {
        match c_string(name) {
            Some(p) => ptrs.push(p.cast_const()),
            None => {
                for p in ptrs {
                    // SAFETY: each p came from CString::into_raw above.
                    drop(unsafe { CString::from_raw(p.cast_mut()) });
                }
                return None;
            }
        }
    }
    let boxed = ptrs.into_boxed_slice();
    let names_len = boxed.len();
    Some(ZeroDdsPartitionQosPolicy {
        names: alloc::boxed::Box::leak(boxed).as_ptr(),
        names_len,
    })
}

/// # Safety
/// `partition` came from [`leak_partition`] and is not released twice.
unsafe fn free_partition(partition: &mut ZeroDdsPartitionQosPolicy) {
    if !partition.names.is_null() && partition.names_len > 0 {
        // SAFETY: names/names_len are the Box<[*const c_char]> leak_partition
        // leaked, each element a CString::into_raw.
        let ptrs = unsafe {
            alloc::boxed::Box::from_raw(slice::from_raw_parts_mut(
                partition.names.cast_mut(),
                partition.names_len,
            ))
        };
        for p in ptrs.iter() {
            // SAFETY: see above.
            drop(unsafe { CString::from_raw(p.cast_mut()) });
        }
    }
    partition.names = ptr::null();
    partition.names_len = 0;
}

/// # Safety
/// `s` came from `CString::into_raw` and is not released twice.
unsafe fn free_c_string(s: &mut *mut c_char) {
    if !s.is_null() {
        // SAFETY: see fn # Safety doc.
        drop(unsafe { CString::from_raw(*s) });
    }
    *s = ptr::null_mut();
}

/// The C form of a `DCPSTopic` sample. `None` when a name holds a NUL
/// that a C string cannot carry; nothing is left allocated then. See
/// [`publication_to_c`] for the other two.
pub(crate) fn topic_to_c(
    s: &zerodds_dcps::builtin_topics::TopicBuiltinTopicData,
) -> Option<ZeroDdsTopicBuiltinTopicData> {
    let mut out = ZeroDdsTopicBuiltinTopicData {
        key: s.key.to_bytes(),
        name: ptr::null_mut(),
        type_name: ptr::null_mut(),
        durability: s.durability.into(),
        durability_service: s.durability_service.into(),
        deadline: s.deadline.into(),
        latency_budget: s.latency_budget.into(),
        liveliness: s.liveliness.into(),
        reliability: s.reliability.into(),
        transport_priority: s.transport_priority.into(),
        lifespan: s.lifespan.into(),
        destination_order: s.destination_order.into(),
        history: s.history.into(),
        resource_limits: s.resource_limits.into(),
        ownership: s.ownership.into(),
        topic_data: leak_octets(&s.topic_data.value),
    };
    let filled = (|| {
        out.name = c_string(&s.name)?;
        out.type_name = c_string(&s.type_name)?;
        Some(())
    })();
    if filled.is_none() {
        // SAFETY: every member was allocated above or is null.
        unsafe { zerodds_topic_builtin_topic_data_free(&mut out) };
        return None;
    }
    Some(out)
}

/// The C form of a `DCPSPublication` sample. `None` when a name holds a
/// NUL that a C string cannot carry; nothing is left allocated then.
pub(crate) fn publication_to_c(
    s: &zerodds_dcps::builtin_topics::PublicationBuiltinTopicData,
) -> Option<ZeroDdsPublicationBuiltinTopicData> {
    let mut out = ZeroDdsPublicationBuiltinTopicData {
        key: s.key.to_bytes(),
        participant_key: s.participant_key.to_bytes(),
        topic_name: ptr::null_mut(),
        type_name: ptr::null_mut(),
        durability: s.durability.into(),
        durability_service: s.durability_service.into(),
        deadline: s.deadline.into(),
        latency_budget: s.latency_budget.into(),
        liveliness: s.liveliness.into(),
        reliability: s.reliability.into(),
        lifespan: s.lifespan.into(),
        user_data: leak_octets(&s.user_data.value),
        ownership: s.ownership.into(),
        ownership_strength: s.ownership_strength.into(),
        destination_order: s.destination_order.into(),
        presentation: s.presentation.into(),
        partition: ZeroDdsPartitionQosPolicy {
            names: ptr::null(),
            names_len: 0,
        },
        topic_data: leak_octets(&s.topic_data.value),
        group_data: leak_octets(&s.group_data.value),
    };
    let filled = (|| {
        out.topic_name = c_string(&s.topic_name)?;
        out.type_name = c_string(&s.type_name)?;
        out.partition = leak_partition(&s.partition.names)?;
        Some(())
    })();
    if filled.is_none() {
        // SAFETY: every member was allocated above or is null.
        unsafe { zerodds_publication_builtin_topic_data_free(&mut out) };
        return None;
    }
    Some(out)
}

/// The C form of a `DCPSSubscription` sample; see [`publication_to_c`].
pub(crate) fn subscription_to_c(
    s: &zerodds_dcps::builtin_topics::SubscriptionBuiltinTopicData,
) -> Option<ZeroDdsSubscriptionBuiltinTopicData> {
    let mut out = ZeroDdsSubscriptionBuiltinTopicData {
        key: s.key.to_bytes(),
        participant_key: s.participant_key.to_bytes(),
        topic_name: ptr::null_mut(),
        type_name: ptr::null_mut(),
        durability: s.durability.into(),
        deadline: s.deadline.into(),
        latency_budget: s.latency_budget.into(),
        liveliness: s.liveliness.into(),
        reliability: s.reliability.into(),
        ownership: s.ownership.into(),
        destination_order: s.destination_order.into(),
        user_data: leak_octets(&s.user_data.value),
        time_based_filter: s.time_based_filter.into(),
        presentation: s.presentation.into(),
        partition: ZeroDdsPartitionQosPolicy {
            names: ptr::null(),
            names_len: 0,
        },
        topic_data: leak_octets(&s.topic_data.value),
        group_data: leak_octets(&s.group_data.value),
    };
    let filled = (|| {
        out.topic_name = c_string(&s.topic_name)?;
        out.type_name = c_string(&s.type_name)?;
        out.partition = leak_partition(&s.partition.names)?;
        Some(())
    })();
    if filled.is_none() {
        // SAFETY: every member was allocated above or is null.
        unsafe { zerodds_subscription_builtin_topic_data_free(&mut out) };
        return None;
    }
    Some(out)
}

/// Releases what a `DCPSTopic` sample filled by the C API holds — the two
/// names and the TOPIC_DATA octet sequence — and leaves those members
/// null. A struct already released, or one the API never filled but
/// zeroed, is left as it is.
///
/// # Safety
/// `data` is null or points at a struct the C API filled.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_builtin_topic_data_free(
    data: *mut ZeroDdsTopicBuiltinTopicData,
) {
    if data.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc.
    unsafe {
        let d = &mut *data;
        free_c_string(&mut d.name);
        free_c_string(&mut d.type_name);
        free_octets(&mut d.topic_data);
    }
}

/// Releases what a `DCPSPublication` sample filled by the C API holds — the
/// two names, the three octet sequences and the partition names — and
/// leaves those members null. A struct already released, or one the API
/// never filled but zeroed, is left as it is.
///
/// # Safety
/// `data` is null or points at a struct the C API filled.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_publication_builtin_topic_data_free(
    data: *mut ZeroDdsPublicationBuiltinTopicData,
) {
    if data.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc.
    unsafe {
        let d = &mut *data;
        free_c_string(&mut d.topic_name);
        free_c_string(&mut d.type_name);
        free_octets(&mut d.user_data);
        free_octets(&mut d.topic_data);
        free_octets(&mut d.group_data);
        free_partition(&mut d.partition);
    }
}

/// The subscription half of [`zerodds_publication_builtin_topic_data_free`].
///
/// # Safety
/// `data` is null or points at a struct the C API filled.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_subscription_builtin_topic_data_free(
    data: *mut ZeroDdsSubscriptionBuiltinTopicData,
) {
    if data.is_null() {
        return;
    }
    // SAFETY: see fn # Safety doc.
    unsafe {
        let d = &mut *data;
        free_c_string(&mut d.topic_name);
        free_c_string(&mut d.type_name);
        free_octets(&mut d.user_data);
        free_octets(&mut d.topic_data);
        free_octets(&mut d.group_data);
        free_partition(&mut d.partition);
    }
}

// ---------------------------------------------------------------------------
// DP → Discovered Topics + Data
// ---------------------------------------------------------------------------

/// Returns the InstanceHandles of all discovered topics.
///
/// The first `min(len, cap)` handles are written into `out`, `*out_count` is
/// set to the **total** number of discovered topics, and the return value is
/// `OutOfResources` when `len > cap` (so a caller retries with a buffer of
/// `*out_count`), `Ok` otherwise. A NULL `out` with `cap == 0` asks for the
/// length alone (`docs/specs/zerodds-c-api-1.0.md` §2.8).
///
/// # Safety
/// `p`, `out_count` valid; `out[0..cap]` writeable, or `out` NULL exactly
/// when `cap == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_discovered_topics(
    p: *mut ZeroDdsDomainParticipant,
    out: *mut u64,
    out_count: *mut usize,
    cap: usize,
) -> c_int {
    if p.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — p+out_count NULL-checked above; the
    // buffer contract is enforced by `copy_list_out`.
    unsafe {
        let pp = &*p;
        let handles: Vec<u64> = pp
            .dp
            .get_discovered_topics()
            .iter()
            .map(InstanceHandle::as_raw)
            .collect();
        copy_list_out(out, cap, &handles, out_count)
    }
}

/// Returns the `ParticipantBuiltinTopicData` for an InstanceHandle.
///
/// # Safety
/// `p`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_discovered_participant_data(
    p: *mut ZeroDdsDomainParticipant,
    handle: u64,
    out: *mut ZeroDdsParticipantBuiltinTopicData,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — p+out NULL-checked above; user_data is lifted into
    // a slice via Box::leak, the caller returns it via builtin_userdata_free.
    unsafe {
        let pp = &*p;
        let data = match pp
            .dp
            .get_discovered_participant_data(InstanceHandle::from_raw(handle))
        {
            Ok(d) => d,
            // DDS 1.4 §2.2.2.2.1.28/.30: a handle of no discovered entity is
            // PRECONDITION_NOT_MET.
            Err(zerodds_dcps::DdsError::PreconditionNotMet { .. }) => {
                return ZeroDdsStatus::PreconditionNotMet as c_int;
            }
            Err(_) => return ZeroDdsStatus::BadParameter as c_int,
        };
        let mut guid = [0u8; 16];
        guid.copy_from_slice(&data.key.to_bytes());
        let bytes = data.user_data.into_boxed_slice();
        let len = bytes.len();
        let p_data = if len == 0 {
            ptr::null()
        } else {
            Box::leak(bytes).as_ptr()
        };
        *out = ZeroDdsParticipantBuiltinTopicData {
            guid,
            user_data: p_data,
            user_data_len: len,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// Returns a previously allocated UserData slice.
///
/// # Safety
/// `p[0..len]` must come from `dp_get_discovered_participant_data`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_userdata_free(p: *const u8, len: usize) {
    if p.is_null() || len == 0 {
        return;
    }
    // SAFETY: see fn # Safety doc — p+len from dp_get_discovered_participant_data
    // Box::leak roundtrip.
    let _ = unsafe { Box::from_raw(slice::from_raw_parts_mut(p as *mut u8, len)) };
}

/// Returns the `TopicBuiltinTopicData` for an InstanceHandle. The names
/// and the TOPIC_DATA octet sequence are copies that belong to the
/// caller until it hands the struct to
/// [`zerodds_topic_builtin_topic_data_free`].
///
/// `ZERODDS_BAD_PARAMETER` for a handle `get_discovered_topics` did not
/// list, `ZERODDS_INVALID_UTF8` when a name holds a NUL and so has no C
/// string; in both cases `out` is left untouched.
///
/// # Safety
/// `p`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_discovered_topic_data(
    p: *mut ZeroDdsDomainParticipant,
    handle: u64,
    out: *mut ZeroDdsTopicBuiltinTopicData,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — p+out NULL-checked above; the names and the
    // octet sequence are the copies zerodds_topic_builtin_topic_data_free releases.
    unsafe {
        let pp = &*p;
        let data = match pp
            .dp
            .get_discovered_topic_data(InstanceHandle::from_raw(handle))
        {
            Ok(d) => d,
            // DDS 1.4 §2.2.2.2.1.28/.30: a handle of no discovered entity is
            // PRECONDITION_NOT_MET.
            Err(zerodds_dcps::DdsError::PreconditionNotMet { .. }) => {
                return ZeroDdsStatus::PreconditionNotMet as c_int;
            }
            Err(_) => return ZeroDdsStatus::BadParameter as c_int,
        };
        let Some(data) = topic_to_c(&data) else {
            // A name with a NUL in it has no C string; nothing is written.
            return ZeroDdsStatus::InvalidUtf8 as c_int;
        };
        *out = data;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Number of discovered publications (Spec §2.2.2.2.1.13).
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_discovered_publications_count(
    p: *mut ZeroDdsDomainParticipant,
) -> usize {
    if p.is_null() {
        return 0;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    unsafe { (*p).dp.discovered_publications_count() }
}

/// Number of discovered subscriptions.
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_discovered_subscriptions_count(
    p: *mut ZeroDdsDomainParticipant,
) -> usize {
    if p.is_null() {
        return 0;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    unsafe { (*p).dp.discovered_subscriptions_count() }
}

// suppress unused-import warning
#[allow(dead_code)]
fn _suppress(_: Vec<u8>) {}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::factory_ffi::{
        zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
    };
    use crate::participant_ffi::zerodds_dp_delete_contained_entities;

    fn mk(domain: u32) -> *mut ZeroDdsDomainParticipant {
        let f = zerodds_dpf_get_instance();
        // SAFETY: f static.
        unsafe { zerodds_dpf_create_participant(f, domain, ptr::null()) }
    }

    fn cleanup(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: p from mk; f static.
        unsafe {
            zerodds_dp_delete_contained_entities(p);
            zerodds_dpf_delete_participant(f, p);
        }
    }

    /// The C form carries every member of the sample — the octet
    /// sequences and the partition names as allocated copies, a duration
    /// to the nanosecond — and the release leaves the pointers null.
    #[test]
    fn a_publication_sample_reaches_c_whole_and_is_released() {
        use zerodds_dcps::builtin_topics::PublicationBuiltinTopicData;
        let sample = PublicationBuiltinTopicData {
            topic_name: "T".into(),
            type_name: "Ty".into(),
            deadline: zerodds_qos::DeadlineQosPolicy {
                period: zerodds_qos::Duration::from_millis(1_250),
            },
            user_data: zerodds_qos::UserDataQosPolicy {
                value: alloc::vec![1, 2, 3],
            },
            partition: zerodds_qos::PartitionQosPolicy {
                names: alloc::vec!["A".into(), "B".into()],
            },
            group_data: zerodds_qos::GroupDataQosPolicy {
                value: alloc::vec![9],
            },
            ..PublicationBuiltinTopicData::default()
        };
        let mut c = publication_to_c(&sample).expect("representable");
        assert_eq!(c.deadline.period.sec, 1);
        assert_eq!(c.deadline.period.nanosec, 250_000_000);
        // SAFETY: the pointers and lengths were just filled by publication_to_c.
        unsafe {
            assert_eq!(
                slice::from_raw_parts(c.user_data.value, c.user_data.value_len),
                &[1, 2, 3]
            );
            assert_eq!(
                slice::from_raw_parts(c.group_data.value, c.group_data.value_len),
                &[9]
            );
            assert!(c.topic_data.value.is_null());
            let names: Vec<&str> = slice::from_raw_parts(c.partition.names, c.partition.names_len)
                .iter()
                .map(|p| core::ffi::CStr::from_ptr(*p).to_str().unwrap())
                .collect();
            assert_eq!(names, ["A", "B"]);
            assert_eq!(
                core::ffi::CStr::from_ptr(c.topic_name).to_str().unwrap(),
                "T"
            );
            zerodds_publication_builtin_topic_data_free(&mut c);
            // A second release finds nothing to free.
            zerodds_publication_builtin_topic_data_free(&mut c);
        }
        assert!(c.topic_name.is_null() && c.type_name.is_null());
        assert!(c.user_data.value.is_null() && c.partition.names.is_null());
    }

    /// A name with a NUL has no C string: the conversion refuses it and
    /// leaves nothing allocated.
    #[test]
    fn a_subscription_name_with_a_nul_is_refused() {
        use zerodds_dcps::builtin_topics::SubscriptionBuiltinTopicData;
        let sample = SubscriptionBuiltinTopicData {
            topic_name: "T".into(),
            partition: zerodds_qos::PartitionQosPolicy {
                names: alloc::vec!["ok".into(), "a\0b".into()],
            },
            user_data: zerodds_qos::UserDataQosPolicy {
                value: alloc::vec![1],
            },
            ..SubscriptionBuiltinTopicData::default()
        };
        assert!(subscription_to_c(&sample).is_none());
    }

    /// Every member of a `DCPSTopic` sample reaches C, including the three
    /// the kind-only struct had no place for — HISTORY, RESOURCE_LIMITS and
    /// TRANSPORT_PRIORITY — as the policy structs `get_qos` already uses; a
    /// duration to the nanosecond, the octet sequence as an allocated copy.
    /// The release empties every pointer it owns.
    #[test]
    fn a_topic_sample_reaches_c_whole_and_is_released() {
        use zerodds_dcps::builtin_topics::TopicBuiltinTopicData;
        let sample = TopicBuiltinTopicData {
            key: TopicBuiltinTopicData::synthesize_key(
                zerodds_rtps::wire_types::GuidPrefix::from_bytes([1; 12]),
                "Chatter",
                "DDS::Chatter",
            ),
            name: "Chatter".into(),
            type_name: "DDS::Chatter".into(),
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: zerodds_qos::DurabilityKind::TransientLocal,
            },
            durability_service: zerodds_qos::DurabilityServiceQosPolicy {
                service_cleanup_delay: zerodds_qos::Duration::from_millis(11),
                history_kind: zerodds_qos::HistoryKind::KeepAll,
                history_depth: -1,
                max_samples: 8,
                max_instances: 4,
                max_samples_per_instance: 2,
            },
            deadline: zerodds_qos::DeadlineQosPolicy {
                period: zerodds_qos::Duration::from_millis(1_250),
            },
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy {
                duration: zerodds_qos::Duration::from_millis(500),
            },
            liveliness: zerodds_qos::LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByParticipant,
                lease_duration: zerodds_qos::Duration::from_secs(2),
            },
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                max_blocking_time: zerodds_qos::Duration::from_millis(75),
            },
            transport_priority: zerodds_qos::TransportPriorityQosPolicy { value: 7 },
            lifespan: zerodds_qos::LifespanQosPolicy {
                duration: zerodds_qos::Duration::from_secs(3),
            },
            destination_order: zerodds_qos::DestinationOrderQosPolicy {
                kind: zerodds_qos::DestinationOrderKind::BySourceTimestamp,
            },
            history: zerodds_qos::HistoryQosPolicy {
                kind: zerodds_qos::HistoryKind::KeepAll,
                depth: -1,
            },
            resource_limits: zerodds_qos::ResourceLimitsQosPolicy {
                max_samples: 4_096,
                max_instances: 32,
                max_samples_per_instance: 128,
            },
            ownership: zerodds_qos::OwnershipQosPolicy {
                kind: zerodds_qos::OwnershipKind::Exclusive,
            },
            topic_data: zerodds_qos::TopicDataQosPolicy {
                value: alloc::vec![1, 2, 3],
            },
        };
        let mut c = topic_to_c(&sample).expect("representable");
        assert_eq!(c.key, sample.key.to_bytes());
        assert_eq!(c.durability.kind, 1);
        assert_eq!(c.durability_service.history_kind, 1);
        assert_eq!(c.durability_service.history_depth, -1);
        assert_eq!(c.durability_service.max_samples_per_instance, 2);
        assert_eq!(c.deadline.period.sec, 1);
        assert_eq!(c.deadline.period.nanosec, 250_000_000);
        assert_eq!(c.latency_budget.duration.nanosec, 500_000_000);
        assert_eq!(c.liveliness.kind, 1);
        assert_eq!(c.liveliness.lease_duration.sec, 2);
        assert_eq!(c.reliability.kind, 2);
        assert_eq!(c.reliability.max_blocking_time.nanosec, 75_000_000);
        assert_eq!(c.transport_priority.value, 7);
        assert_eq!(c.lifespan.duration.sec, 3);
        assert_eq!(c.destination_order.kind, 1);
        assert_eq!(c.history.kind, 1);
        assert_eq!(c.history.depth, -1);
        assert_eq!(c.resource_limits.max_samples, 4_096);
        assert_eq!(c.resource_limits.max_instances, 32);
        assert_eq!(c.resource_limits.max_samples_per_instance, 128);
        assert_eq!(c.ownership.kind, 1);
        // SAFETY: the pointers and lengths were just filled by topic_to_c.
        unsafe {
            assert_eq!(
                core::ffi::CStr::from_ptr(c.name).to_str().unwrap(),
                "Chatter"
            );
            assert_eq!(
                core::ffi::CStr::from_ptr(c.type_name).to_str().unwrap(),
                "DDS::Chatter"
            );
            assert_eq!(
                slice::from_raw_parts(c.topic_data.value, c.topic_data.value_len),
                &[1, 2, 3]
            );
            zerodds_topic_builtin_topic_data_free(&mut c);
            // A second release finds nothing to free.
            zerodds_topic_builtin_topic_data_free(&mut c);
        }
        assert!(c.name.is_null() && c.type_name.is_null());
        assert!(c.topic_data.value.is_null() && c.topic_data.value_len == 0);
    }

    /// A topic name with a NUL has no C string: the conversion refuses it,
    /// leaves nothing allocated, and the getter reports it as
    /// `ZERODDS_INVALID_UTF8` rather than filling an empty name.
    #[test]
    fn a_topic_name_with_a_nul_is_refused() {
        use zerodds_dcps::builtin_topics::TopicBuiltinTopicData;
        let sample = TopicBuiltinTopicData {
            name: "a\0b".into(),
            topic_data: zerodds_qos::TopicDataQosPolicy {
                value: alloc::vec![1],
            },
            ..TopicBuiltinTopicData::default()
        };
        assert!(topic_to_c(&sample).is_none());
    }

    /// A handle `get_discovered_topics` did not list fills nothing: the
    /// caller must not read the struct, and must not release it.
    #[test]
    fn discovered_topic_data_unknown_handle_returns_error() {
        let p = mk(64);
        // SAFETY: every member is an integer, an array of them or a raw
        // pointer; all-zero is a valid value of each.
        let mut data = unsafe { core::mem::zeroed::<ZeroDdsTopicBuiltinTopicData>() };
        // SAFETY: p from mk; data stack-local, zeroed.
        let rc = unsafe { zerodds_dp_get_discovered_topic_data(p, 0xDEAD, &mut data) };
        assert_eq!(rc, ZeroDdsStatus::PreconditionNotMet as c_int);
        assert!(data.name.is_null());
        // SAFETY: the two names are null; nothing was allocated.
        unsafe { zerodds_topic_builtin_topic_data_free(&mut data) };
        cleanup(p);
    }

    #[test]
    fn discovered_topics_empty_initially() {
        let p = mk(61);
        let mut buf = [0u64; 16];
        let mut count = 0usize;
        // SAFETY: p from mk; buf+count stack-local.
        let rc = unsafe { zerodds_dp_get_discovered_topics(p, buf.as_mut_ptr(), &mut count, 16) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(count, 0);
        cleanup(p);
    }

    /// `cap == 0` with a NULL buffer asks for the total length; a NULL buffer
    /// with `cap > 0` is refused (`docs/specs/zerodds-c-api-1.0.md` §2.8).
    #[test]
    fn discovered_topics_length_query_and_null_contract() {
        let p = mk(65);
        let mut count = 0usize;
        // SAFETY: p from mk; count stack-local.
        unsafe {
            let rc = zerodds_dp_get_discovered_topics(p, ptr::null_mut(), &mut count, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 0);
            let rc = zerodds_dp_get_discovered_topics(p, ptr::null_mut(), &mut count, 8);
            assert_eq!(rc, ZeroDdsStatus::BadParameter as c_int);
        }
        cleanup(p);
    }

    #[test]
    fn discovered_publications_count_starts_zero() {
        let p = mk(62);
        // SAFETY: p from mk.
        unsafe {
            assert_eq!(zerodds_dp_discovered_publications_count(p), 0);
            assert_eq!(zerodds_dp_discovered_subscriptions_count(p), 0);
        }
        cleanup(p);
    }

    #[test]
    fn discovered_participant_data_unknown_handle_returns_error() {
        let p = mk(63);
        let mut data = ZeroDdsParticipantBuiltinTopicData {
            guid: [0u8; 16],
            user_data: ptr::null(),
            user_data_len: 0,
        };
        // SAFETY: p from mk; data stack-local.
        let rc = unsafe { zerodds_dp_get_discovered_participant_data(p, 0xDEAD, &mut data) };
        assert_eq!(rc, ZeroDdsStatus::PreconditionNotMet as c_int);
        cleanup(p);
    }
}
