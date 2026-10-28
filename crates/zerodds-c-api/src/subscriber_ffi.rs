// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Subscriber + DataReader C-FFI (Spec §2.2.2.5 + DDS-PSM-Cxx §7.5.6/§7.5.8).

use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ffi::c_int;
use core::ptr;
use core::slice;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use zerodds_dcps::instance_handle::InstanceHandle;
use zerodds_dcps::qos::{DataReaderQos, ReliabilityKind};
use zerodds_dcps::runtime::{UserReaderConfig, UserSample};
use zerodds_dcps::sample_info::{
    InstanceStateKind, SampleInfo as DcpsSampleInfo, SampleStateKind, ViewStateKind,
};
use zerodds_rtps::wire_types::TopicKind;

use crate::ZeroDdsStatus;
use crate::entities::{
    CftFilter, ZeroDdsDataReader, ZeroDdsDomainParticipant, ZeroDdsSubscriber, ZeroDdsTopic,
};
use crate::ffi_helpers::copy_list_out;
use crate::qos_ffi::{ZeroDdsDataReaderQos, dr_qos_from_c_checked};

// ---------------------------------------------------------------------------
// Subscriber
// ---------------------------------------------------------------------------

/// Returns the owning participant.
///
/// # Safety
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_get_participant(
    sub: *mut ZeroDdsSubscriber,
) -> *mut ZeroDdsDomainParticipant {
    if sub.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — sub NULL-checked above.
    unsafe { (*sub).participant }
}

/// `begin_access` (DDS 1.4 §2.2.2.5.2.8): "the application is about to
/// access the data samples in any of the DataReader objects attached to
/// the Subscriber". Under PRESENTATION `access_scope` GROUP the
/// sample-accessing operations need it: without an open `begin_access`,
/// read, take, their `_w_condition` forms and `get_datareaders` return
/// `ZERODDS_PRECONDITION_NOT_MET`, except inside a data listener. The calls
/// nest.
///
/// # Safety
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_begin_access(sub: *mut ZeroDdsSubscriber) -> c_int {
    // SAFETY: a non-null `sub` is a live subscriber handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(sub) } {
        return rc;
    }
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — sub NULL-checked above.
    unsafe { (*sub).access.begin() };
    ZeroDdsStatus::Ok as c_int
}

/// `end_access` (DDS 1.4 §2.2.2.5.2.9): "Calls to begin_access and
/// end_access must be properly nested and paired"; without an open
/// `begin_access` `ZERODDS_PRECONDITION_NOT_MET`.
///
/// # Safety
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_end_access(sub: *mut ZeroDdsSubscriber) -> c_int {
    // SAFETY: a non-null `sub` is a live subscriber handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(sub) } {
        return rc;
    }
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — sub NULL-checked above.
    match unsafe { (*sub).access.end() } {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(_) => ZeroDdsStatus::PreconditionNotMet as c_int,
    }
}

/// DDS 1.4 §2.2.2.5.2.8: under PRESENTATION `access_scope` GROUP "the
/// operation begin_access must be called prior to calling any of the
/// sample-accessing operations, namely: get_datareaders on the Subscriber
/// and read, take, read_w_condition, take_w_condition on any DataReader.
/// Otherwise the sample-accessing operations will return the error
/// PRECONDITION_NOT_MET." A data listener needs none, as in the typed API.
pub(crate) fn group_access_allowed(sb: Option<&ZeroDdsSubscriber>) -> bool {
    let Some(sb) = sb else {
        return true;
    };
    let group = sb
        .qos
        .lock()
        .is_ok_and(|q| q.presentation.access_scope == zerodds_qos::PresentationAccessScope::Group);
    !group || sb.access.is_active() || crate::listener_ffi::in_data_listener()
}

/// `get_datareaders` (DDS 1.4 §2.2.2.5.2.10): the DataReaders of the
/// subscriber that hold samples in the three state masks (0: any state of
/// that kind). Under PRESENTATION `access_scope` GROUP with
/// `ordered_access` a reader once per sample, in the order the samples
/// were written across the readers (by source timestamp, a tie to the
/// older reader); otherwise each reader once, in creation order. Under
/// GROUP scope the call needs `begin_access`. It returned every reader,
/// with samples or not.
///
/// The first `min(len, cap)` reader handles are written into `out`,
/// `*out_count` is set to the **total**, and the return value is
/// `OutOfResources` when `len > cap` (so a caller retries with a buffer of
/// `*out_count`), `Ok` otherwise. A NULL `out` with `cap == 0` asks for the
/// length alone (`docs/specs/zerodds-c-api-1.0.md` §2.8).
///
/// # Safety
/// `sub`, `out_count` valid; `out[0..cap]` writeable, or `out` NULL exactly
/// when `cap == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_get_datareaders(
    sub: *mut ZeroDdsSubscriber,
    out: *mut *mut ZeroDdsDataReader,
    out_count: *mut usize,
    cap: usize,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
) -> c_int {
    // SAFETY: a non-null `sub` is a live subscriber handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(sub) } {
        return rc;
    }
    if sub.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — sub+out_count NULL-checked above; the
    // readers are valid while the subscriber lists them; the buffer
    // contract is enforced by `copy_list_out`.
    unsafe {
        let sb = &*sub;
        if !group_access_allowed(Some(sb)) {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        let states = (sample_states, view_states, instance_states);
        let drs = sb.datareaders.lock().map(|g| g.clone()).unwrap_or_default();
        let mut held: Vec<(*mut ZeroDdsDataReader, Vec<zerodds_dcps::Time>)> = Vec::new();
        for dr in drs {
            if dr.is_null() {
                continue;
            }
            let drr = &*dr;
            apply_lost_writers(drr);
            if let Err(status) = drain_fresh_into_read_cache(drr, usize::MAX) {
                return status as c_int;
            }
            let times: Vec<zerodds_dcps::Time> = match drr.read_cache.lock() {
                Ok(cache) => cache
                    .iter()
                    .filter(|e| entry_in_states(e.1, &e.2, states))
                    .map(|e| e.2.source_timestamp)
                    .collect(),
                Err(_) => return ZeroDdsStatus::Error as c_int,
            };
            if !times.is_empty() {
                held.push((dr, times));
            }
        }
        let list: Vec<*mut ZeroDdsDataReader> =
            if ReaderPresentation::of(Some(sb)).order == ReadOrder::OnePerRead {
                in_write_order(&held)
            } else {
                held.into_iter().map(|(dr, _)| dr).collect()
            };
        copy_list_out(out, cap, &list, out_count)
    }
}

/// A merge of the readers' sample sequences by source timestamp: a reader
/// once per sample, a tie to the earlier reader.
fn in_write_order(
    held: &[(*mut ZeroDdsDataReader, Vec<zerodds_dcps::Time>)],
) -> Vec<*mut ZeroDdsDataReader> {
    let mut next = alloc::vec![0usize; held.len()];
    let total: usize = held.iter().map(|(_, times)| times.len()).sum();
    let mut out = Vec::with_capacity(total);
    while out.len() < total {
        let mut pick: Option<usize> = None;
        for (i, (_, times)) in held.iter().enumerate() {
            let Some(t) = times.get(next[i]) else {
                continue;
            };
            if pick.is_none_or(|p| *t < held[p].1[next[p]]) {
                pick = Some(i);
            }
        }
        let Some(i) = pick else {
            break;
        };
        next[i] += 1;
        out.push(held[i].0);
    }
    out
}

/// `notify_datareaders` (DDS 1.4 §2.2.2.5.2.14): calls `on_data_available`
/// of every DataReader of the subscriber whose DATA_AVAILABLE changed
/// since its listener was last called (§2.2.4.2.2), on this thread; the
/// pattern of `on_data_on_readers`.
///
/// # Safety
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_notify_datareaders(sub: *mut ZeroDdsSubscriber) -> c_int {
    // SAFETY: a non-null `sub` is a live subscriber handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(sub) } {
        return rc;
    }
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — sub NULL-checked above.
    unsafe { crate::listener_ffi::notify_datareaders(sub) };
    ZeroDdsStatus::Ok as c_int
}

/// Creates a DataReader over a **keyed** topic.
///
/// The topic kind is what an untyped reader knows about its type's keys, so
/// a keyless topic must be created through
/// [`zerodds_sub_create_datareader_kind`] with `is_keyed = 0` — otherwise the
/// take path resolves an instance per distinct payload instead of the single
/// instance a keyless topic has (DDS 1.4 §2.2.1.2.2).
///
/// # Safety
/// `sub`, `topic` valid; `qos` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_datareader(
    sub: *mut ZeroDdsSubscriber,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataReaderQos,
) -> *mut ZeroDdsDataReader {
    // SAFETY: forwarded unchanged — same contract as this function's own.
    unsafe { zerodds_sub_create_datareader_kind(sub, topic, qos, 1) }
}

/// Creates a DataReader with an explicit keyed / no-keyed flag — the topic
/// surface's counterpart to [`crate::zerodds_reader_create_kind`].
///
/// `is_keyed = 0` when the IDL type has no `@key` member. Two things follow
/// from it, both of them spec text rather than a tuning knob:
///
/// * the reader endpoint's `entityKind` becomes `NO_KEY` instead of
///   `WITH_KEY` (DDSI-RTPS 2.5 §9.3.1.2 Table 9.1 — the endpoint kind follows
///   the type's keyedness);
/// * `zerodds_dr_take` / `zerodds_dr_read` report the topic's **single**
///   instance (DDS 1.4 §2.2.1.2.2) for every sample, with the all-zero
///   KeyHash of an empty Key Holder (DDS-XTypes 1.3 §7.6.8) and thus
///   `instance_handle = HANDLE_NIL`, instead of deriving an instance from
///   each payload.
///
/// # Safety
/// `sub`, `topic` valid; `qos` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_datareader_kind(
    sub: *mut ZeroDdsSubscriber,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataReaderQos,
    is_keyed: c_int,
) -> *mut ZeroDdsDataReader {
    // SAFETY: forwarded unchanged — same contract as this function's own.
    unsafe { create_datareader(sub, topic, qos, is_keyed, None) }
}

/// The reader behind `zerodds_sub_create_datareader_kind`, with the
/// ContentFilteredTopic its SEDP subscription announces, if any.
///
/// # Safety
/// As [`zerodds_sub_create_datareader_kind`].
unsafe fn create_datareader(
    sub: *mut ZeroDdsSubscriber,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataReaderQos,
    is_keyed: c_int,
    content_filter: Option<zerodds_rtps::subscription_data::ContentFilterProperty>,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || topic.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — sub+topic NULL-checked above; participant
    // from dp_create_subscriber; qos NULL-tolerant.
    unsafe {
        let sb = &*sub;
        let dp_handle = sb.participant;
        if dp_handle.is_null() {
            return ptr::null_mut();
        }
        let dp = &*dp_handle;
        let rt = match dp.rt.as_ref() {
            Some(r) => r.clone(),
            None => return ptr::null_mut(),
        };

        let qos: DataReaderQos = if qos.is_null() {
            sb.default_dr_qos
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default()
        } else {
            match dr_qos_from_c_checked(qos) {
                Ok(q) => q,
                Err(_) => return ptr::null_mut(),
            }
        };

        register_datareader(
            sub,
            topic,
            rt,
            qos,
            is_keyed != 0,
            zerodds_types::TypeIdentifier::default(),
            content_filter,
        )
    }
}

/// Creates a DataReader over the topic that advertises its type via a
/// serialized COMPLETE `TypeObject` (F-TYPES-3 / #24). Topic-path mirror of the
/// flat [`crate::zerodds_reader_create_typed`] and reader-side twin of
/// [`zerodds_pub_create_datawriter_typed`](crate::publisher_ffi::zerodds_pub_create_datawriter_typed): deserializes `type_object_bytes`,
/// registers the object into the shared TypeLookup registry, derives the
/// strongly-hashed `TypeIdentifier` and sets the reader slot's
/// `type_identifier` from it before the endpoint is published — no `None`-window
/// against discovery. This is the variant the C#/Java bindings call.
///
/// # Safety
/// Like [`zerodds_sub_create_datareader`], plus: `type_object_bytes` must point
/// to `len` readable bytes (NULL / `len == 0` is rejected with NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_datareader_typed(
    sub: *mut ZeroDdsSubscriber,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataReaderQos,
    type_object_bytes: *const u8,
    len: usize,
) -> *mut ZeroDdsDataReader {
    // SAFETY: forwards the existing caller contract unchanged.
    unsafe { create_datareader_typed(sub, topic, qos, type_object_bytes, len, None) }
}

/// Creates a typed reader accepting the generated codec's representation only.
/// `is_keyed` is 0 or 1; `representation` is 0 (XCDR1) or 2 (XCDR2).
/// Both wire byte orders may arrive and are reported with each sample.
/// Invalid enum values return NULL without registering an endpoint.
///
/// # Safety
/// Same pointer/lifetime requirements as [`zerodds_sub_create_datareader_typed`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_datareader_typed_wire(
    sub: *mut ZeroDdsSubscriber,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataReaderQos,
    type_object_bytes: *const u8,
    len: usize,
    is_keyed: c_int,
    representation: c_int,
) -> *mut ZeroDdsDataReader {
    let repr = match representation {
        0 => 0_i16,
        2 => 2_i16,
        _ => return ptr::null_mut(),
    };
    if !matches!(is_keyed, 0 | 1) {
        return ptr::null_mut();
    }
    // SAFETY: forwards the documented pointer contract after enum validation.
    unsafe {
        create_datareader_typed(
            sub,
            topic,
            qos,
            type_object_bytes,
            len,
            Some((is_keyed == 1, repr)),
        )
    }
}

// SAFETY: `sub` and `topic` must each be null or point to a valid
// ZeroDdsSubscriber/ZeroDdsTopic; `qos` must be null or readable for
// size_of::<ZeroDdsDataReaderQos>(); `type_object_bytes` must point to `len`
// readable bytes for the caller's registration pledge; `wire` carries a
// representation pair the caller already validated. Nulls and the missing
// runtime are refused below before any dereference.
unsafe fn create_datareader_typed(
    sub: *mut ZeroDdsSubscriber,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataReaderQos,
    type_object_bytes: *const u8,
    len: usize,
    wire: Option<(bool, i16)>,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || topic.is_null() || type_object_bytes.is_null() || len == 0 {
        return ptr::null_mut();
    }
    // SAFETY: the contract above holds; every pointer was null-checked
    // before this block and the participant handle is re-checked below.
    unsafe {
        let sb = &*sub;
        let dp_handle = sb.participant;
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

        // The entityKind follows the type's keyedness (Table 9.1) unless the
        // caller states it.
        let keyed = wire.map_or_else(|| crate::type_object_keyed_from_bytes(&rt, bytes), |w| w.0);

        let mut qos: DataReaderQos = if qos.is_null() {
            sb.default_dr_qos
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default()
        } else {
            match dr_qos_from_c_checked(qos) {
                Ok(q) => q,
                Err(_) => return ptr::null_mut(),
            }
        };

        if let Some((_, repr)) = wire {
            qos.data_representation = Some(alloc::vec![repr]);
        }

        register_datareader(
            sub,
            topic,
            rt,
            qos,
            keyed,
            // F-TYPES-3 / #24: derived from the registered TypeObject (not None).
            type_identifier,
            None,
        )
    }
}

/// What both reader constructors share once the QoS and the type's
/// identity are decided: the configuration the runtime announces, the
/// registration, and the handle the C caller gets, noted with its
/// subscriber.
///
/// # Safety
/// `sub` and `topic` are the non-null subscriber and topic the caller
/// checked.
unsafe fn register_datareader(
    sub: *mut ZeroDdsSubscriber,
    topic: *mut ZeroDdsTopic,
    rt: std::sync::Arc<zerodds_dcps::runtime::DcpsRuntime>,
    qos: DataReaderQos,
    keyed: bool,
    type_identifier: zerodds_types::TypeIdentifier,
    content_filter: Option<zerodds_rtps::subscription_data::ContentFilterProperty>,
) -> *mut ZeroDdsDataReader {
    // SAFETY: the caller's contract — both pointers are valid and non-null.
    let sb = unsafe { &*sub };
    // §9.3.1.2 Table 9.1: the endpoint's entityKind follows the type's
    // keyedness, and the take path reads the topic kind back out of it. The
    // reader exists, with its id and its channel, from here; the runtime
    // registers it — announces and matches it — when it is enabled (DDS 1.4
    // §2.2.2.1.1.7, §2.2.3.20).
    let eid = rt.reserve_user_reader_id(keyed);
    let (tx, rx) = mpsc::channel();
    let ownership = qos.ownership.kind;
    let dr = Box::new(ZeroDdsDataReader {
        subscriber: sub,
        topic,
        rt,
        eid,
        qos: Mutex::new(qos),
        rx: Mutex::new(rx),
        coherent_hold: Default::default(),
        group_inbox: Default::default(),
        read_cache: Mutex::new(Vec::new()),
        held_back: Mutex::new(Vec::new()),
        cft_filter: None,
        cft_hold: None,
        query_type: Mutex::new(None),
        ownership,
        instances: zerodds_dcps::instance_tracker::InstanceTracker::new(),
        partition_out: Mutex::new(Default::default()),
        lifecycle: crate::entity_ffi::Lifecycle::new(false),
    });
    let dr_ptr = Box::into_raw(dr);
    crate::entities::note_status_entity(dr_ptr, crate::entities::StatusEntityKind::DataReader);
    // SAFETY: `dr_ptr` was just allocated.
    unsafe {
        (*dr_ptr).lifecycle.set_registration(
            reader_registration(dr_ptr as usize, tx, type_identifier),
            content_filter,
        );
    }
    if let Ok(mut list) = sb.datareaders.lock() {
        list.push(dr_ptr);
    }
    // SAFETY: `dr_ptr` was just allocated; its subscriber is `sb`.
    unsafe {
        if crate::entity_ffi::subscriber_autoenables(sb) && (*dr_ptr).lifecycle.enable().is_err() {
            if let Ok(mut list) = sb.datareaders.lock() {
                list.retain(|x| *x != dr_ptr);
            }
            drop(Box::from_raw(dr_ptr));
            return ptr::null_mut();
        }
    }
    dr_ptr
}

/// The registration of the reader at `dr`: the runtime gets the reader under
/// its reserved id with the QoS the reader has when it is enabled and the
/// content filter its ContentFilteredTopic has then, and the Subscriber's
/// GROUP order learns its inbox.
fn reader_registration(
    dr: usize,
    tx: mpsc::Sender<UserSample>,
    type_identifier: zerodds_types::TypeIdentifier,
) -> crate::entity_ffi::Registration {
    Box::new(move |content_filter| {
        // SAFETY: the registration runs from the handle's own lifecycle, so
        // the handle at `dr` is alive; its subscriber and topic outlive it.
        let (drr, sb, tt) = unsafe {
            let drr = &*(dr as *const ZeroDdsDataReader);
            (drr, &*drr.subscriber, &*drr.topic)
        };
        let rt = &drr.rt;
        let eid = drr.eid;
        let qos: DataReaderQos = drr
            .qos
            .lock()
            .map(|g| g.clone())
            .map_err(|_| ZeroDdsStatus::PreconditionNotMet)?;
        let scoped = subscriber_scoped(sb, &qos);
        let cfg = UserReaderConfig {
            topic_name: tt.name.to_string(),
            type_name: tt.type_name.to_string(),
            reliable: matches!(qos.reliability.kind, ReliabilityKind::Reliable),
            durability: qos.durability.kind,
            deadline: qos.deadline.clone(),
            // Announced and decided on as the application set them; all
            // four were left at their defaults (DDS 1.4 §2.2.3.12,
            // §2.2.3.8, §2.2.3.17; XTypes 1.3 §7.6.3.4.4).
            time_based_filter: qos.time_based_filter,
            latency_budget: qos.latency_budget,
            destination_order: qos.destination_order,
            type_consistency: qos.type_consistency.clone(),
            liveliness: qos.liveliness.clone(),
            ownership: qos.ownership.kind,
            // PARTITION, PRESENTATION and GROUP_DATA are the Subscriber's
            // (DDS 1.4 §2.2.3.13, §2.2.3.6, §2.2.3.3); `subscriber_scoped`
            // says which of the two QoS each comes from.
            partition: scoped.partition,
            presentation: scoped.presentation,
            user_data: qos.user_data.value.clone(),
            // TOPIC_DATA is the Topic's (DDS 1.4 §2.2.3.2, §2.2.5): the endpoint announces the
            // Topic's value unless its own QoS carries one.
            topic_data: if qos.topic_data.value.is_empty() {
                tt.qos
                    .lock()
                    .map(|topic_qos| topic_qos.topic_data.value.clone())
                    .unwrap_or_default()
            } else {
                qos.topic_data.value.clone()
            },
            group_data: scoped.group_data,
            type_identifier: type_identifier.clone(),
            data_representation_offer: qos.data_representation.clone(),
            content_filter,
            group_entity_id: sb.entity_id,
            ..Default::default()
        };
        rt.register_user_reader_as(
            eid,
            cfg,
            zerodds_dcps::runtime::GroupTopicQos::default(),
            tx.clone(),
        )
        .map_err(|_| ZeroDdsStatus::Error)?;
        // A C reader has no DCPS reader in front of it: the runtime applies
        // its TIME_BASED_FILTER.
        rt.set_user_reader_time_based_filter(eid, time_based_filter_nanos(&qos));
        if let Ok(mut inboxes) = sb.group.inboxes.lock() {
            inboxes.push((eid, alloc::sync::Arc::downgrade(&drr.group_inbox)));
        }
        if drr.lifecycle.waker_wanted() {
            crate::condition_ffi::register_reader_data_waker(drr);
        }
        Ok(())
    })
}

/// Creates a DataReader on a ContentFilteredTopic.
/// On every `take`/`read` the filter expression is evaluated
/// (Spec §2.2.2.3.3 + §2.2.2.5.2.5).
///
/// # Safety
/// `sub`, `cft` valid; `qos` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_datareader_with_cft(
    sub: *mut ZeroDdsSubscriber,
    cft: *mut crate::entities::ZeroDdsContentFilteredTopic,
    qos: *const ZeroDdsDataReaderQos,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || cft.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — sub+cft NULL-checked above; cft from
    // create_contentfilteredtopic; delegation to zerodds_sub_create_datareader;
    // dr is a fresh Box::into_raw that we collect again to set cft_filter.
    unsafe {
        let cft_ref = &*cft;
        let related_topic = cft_ref.related_topic;
        if related_topic.is_null() {
            return ptr::null_mut();
        }
        let expr = match zerodds_sql_filter::parse(&cft_ref.filter_expression) {
            Ok(e) => e,
            Err(_) => return ptr::null_mut(),
        };
        let params = std::sync::Arc::clone(&cft_ref.values);

        let schema: Vec<crate::entities::CftField> =
            cft_ref.schema.lock().map(|g| g.clone()).unwrap_or_default();
        let extensibility = cft_ref.extensibility.lock().map(|g| *g).unwrap_or_default();
        let typed = cft_ref.typed.lock().ok().and_then(|g| g.clone());
        // A filter that reads a field needs to know where the field is: the
        // type (`zerodds_cft_set_type_object`) or a positional schema
        // (`zerodds_cft_set_schema`). Without either every sample would pass
        // unfiltered, so the reader is refused.
        if expr.references_fields() && typed.is_none() && schema.is_empty() {
            return ptr::null_mut();
        }

        let params_raw = cft_ref
            .parameters
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let property = zerodds_rtps::subscription_data::ContentFilterProperty {
            content_filtered_topic_name: cft_ref.name.clone(),
            related_topic_name: (*related_topic).name.clone(),
            filter_class_name: zerodds_rtps::subscription_data::filter_class::DDSSQL.into(),
            filter_expression: cft_ref.filter_expression.clone(),
            expression_parameters: params_raw,
        };
        let dr = create_datareader(sub, related_topic, qos, 1, Some(property));
        if dr.is_null() {
            return ptr::null_mut();
        }
        let mut boxed = Box::from_raw(dr);
        if let Ok(mut readers) = cft_ref.readers.lock() {
            readers.push((std::sync::Arc::clone(&boxed.rt), boxed.eid));
        }
        boxed.cft_hold = Some(crate::entities::CftReaderHold::new(&cft_ref.live_readers));
        boxed.cft_filter = Some(crate::entities::CftFilter {
            expr,
            params,
            schema,
            extensibility,
            typed,
        });
        Box::into_raw(boxed)
    }
}

/// Deletes a DataReader.
///
/// # Safety
/// `sub`, `dr` valid and belonging together.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_delete_datareader(
    sub: *mut ZeroDdsSubscriber,
    dr: *mut ZeroDdsDataReader,
) -> c_int {
    if sub.is_null() || dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — sub+dr NULL-checked above; dr from sub_create_datareader.
    unsafe {
        if (*dr).subscriber != sub {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        let sb = &*sub;
        if let Ok(mut list) = sb.datareaders.lock() {
            let n = list.len();
            list.retain(|x| *x != dr);
            if list.len() == n {
                return ZeroDdsStatus::BadHandle as c_int;
            }
        }
        // Drop any zero-copy SHM map state keyed by this reader's (runtime, eid).
        #[cfg(feature = "flatdata-loan")]
        {
            let drr = &*dr;
            crate::shm_loan_ffi::forget_reader(&drr.rt, drr.eid);
        }
        let _ = Box::from_raw(dr);
    }
    ZeroDdsStatus::Ok as c_int
}

/// Look up a DataReader by topic name.
///
/// # Safety
/// `sub`, `topic_name` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_lookup_datareader(
    sub: *mut ZeroDdsSubscriber,
    topic_name: *const core::ffi::c_char,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — sub+topic_name NULL-checked above; datareaders
    // and their topic pointers from sub_create_datareader (Box::into_raw).
    unsafe {
        let cs = std::ffi::CStr::from_ptr(topic_name);
        let name = match cs.to_str() {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        };
        let sb = &*sub;
        if let Ok(list) = sb.datareaders.lock() {
            for &dr in list.iter() {
                if dr.is_null() {
                    continue;
                }
                let drr = &*dr;
                if !drr.topic.is_null() && (*drr.topic).name == name {
                    return dr;
                }
            }
        }
    }
    ptr::null_mut()
}

/// Deletes all DataReaders held by the subscriber.
///
/// # Safety
/// `sub` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_delete_contained_entities(
    sub: *mut ZeroDdsSubscriber,
) -> c_int {
    if sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — sub NULL-checked above; datareaders from
    // sub_create_datareader (Box::into_raw).
    unsafe {
        let drs: Vec<*mut ZeroDdsDataReader> = (*sub)
            .datareaders
            .lock()
            .map(|mut g| core::mem::take(&mut *g))
            .unwrap_or_default();
        for dr in drs {
            if !dr.is_null() {
                let _ = Box::from_raw(dr);
            }
        }
    }
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// DataReader: read / take + Sample-API
// ---------------------------------------------------------------------------

/// Returns the TopicDescription handle (in RC1: the topic pointer).
///
/// # Safety
/// `dr` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_topicdescription(
    dr: *mut ZeroDdsDataReader,
) -> *mut ZeroDdsTopic {
    if dr.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — dr NULL-checked above.
    unsafe { (*dr).topic }
}

/// Returns the subscriber.
///
/// # Safety
/// `dr` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_subscriber(
    dr: *mut ZeroDdsDataReader,
) -> *mut ZeroDdsSubscriber {
    if dr.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — dr NULL-checked above.
    unsafe { (*dr).subscriber }
}

/// SampleInfo (Spec §2.2.2.5.4 + DDS-PSM-Cxx §7.5.8.1).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsSampleInfo {
    /// Sample-state bit (1=READ, 2=NOT_READ).
    pub sample_state: u32,
    /// View state (1=NEW, 2=NOT_NEW).
    pub view_state: u32,
    /// Instance state (1=ALIVE, 2=NOT_ALIVE_DISPOSED, 4=NOT_ALIVE_NO_WRITERS).
    pub instance_state: u32,
    /// disposed_generation_count.
    pub disposed_generation_count: i32,
    /// no_writers_generation_count.
    pub no_writers_generation_count: i32,
    /// sample_rank.
    pub sample_rank: i32,
    /// generation_rank.
    pub generation_rank: i32,
    /// absolute_generation_rank.
    pub absolute_generation_rank: i32,
    /// source_timestamp seconds.
    pub source_timestamp_sec: i32,
    /// source_timestamp nanoseconds.
    pub source_timestamp_nanosec: u32,
    /// Instance handle.
    pub instance_handle: u64,
    /// Publication handle (writer-GUID hash).
    pub publication_handle: u64,
    /// `true` if the payload actually has data (vs. a lifecycle marker).
    pub valid_data: bool,
    /// XCDR version of the payload from the encapsulation header (RTPS 2.5
    /// §10.5): `0` = XCDR1 (CDR/PL_CDR), `1` = XCDR2 (CDR2/D_CDR2/PL_CDR2).
    /// The typed consumer needs this to apply the correct alignment rule.
    /// `0` for lifecycle markers (no payload).
    pub representation: u8,
    /// Wire byte order of the payload: `0` = little-endian (the canonical
    /// XCDR2 wire), `1` = big-endian — from the encapsulation representation
    /// identifier's low bit. The typed consumer dispatches its little-endian
    /// vs big-endian decoder on this. `0` for lifecycle markers.
    pub big_endian: u8,
}

/// Sample array (Spec §5 mini-spec).
#[repr(C)]
pub struct ZeroDdsSampleArray {
    /// Array of payload pointers.
    pub buffers: *mut *mut u8,
    /// Array of payload lengths.
    pub lengths: *mut usize,
    /// Array of SampleInfos.
    pub infos: *mut ZeroDdsSampleInfo,
    /// Number of samples.
    pub count: usize,
    /// Internal loan token (pointer to a `Vec<UserSample>` box). Freed
    /// by `return_loan`.
    pub loan_token: *mut core::ffi::c_void,
}

/// `SampleInfo.source_timestamp` (DDS 1.4 §2.2.2.5.5), baked from the
/// INFO_TS a sample arrived with — `None` (no INFO_TS, e.g. a same-runtime
/// intra-process dispatch or a durability replay) becomes the spec's
/// `TIME_INVALID` sentinel, not the epoch, which is a time a writer could
/// have legitimately stamped.
fn baked_source_timestamp(
    ts: Option<zerodds_rtps::header_extension::HeTimestamp>,
) -> zerodds_dcps::time::Time {
    ts.map(zerodds_dcps::time::he_timestamp_to_time)
        .unwrap_or(zerodds_dcps::time::Time::INVALID)
}

/// `SampleInfo.source_timestamp` as the C struct's two fields.
fn time_parts(t: zerodds_dcps::time::Time) -> (i32, u32) {
    (t.sec, t.nanosec)
}

/// Internal loan memory: held until `return_loan` is called.
struct LoanMemory {
    payloads: Vec<Vec<u8>>,
    buffers: Vec<*mut u8>,
    lengths: Vec<usize>,
    infos: Vec<ZeroDdsSampleInfo>,
}

impl LoanMemory {
    /// Builds the C-ABI loan from a batch of already-resolved samples.
    /// `resolve_instances_and_ownership` bakes each `DcpsSampleInfo` exactly
    /// once, at arrival (see the `read_cache` field doc on
    /// [`crate::entities::ZeroDdsDataReader`] for why) — this only converts
    /// the baked value to the `repr(C)` struct and fills the three fields
    /// that describe the returned collection as a whole rather than a
    /// single sample.
    ///
    /// `tracker` is the reader's `InstanceTracker`, read ONLY to compute
    /// `sample_rank`/`generation_rank`/`absolute_generation_rank`
    /// (`zerodds_dcps::sample_info::compute_ranks`, DDS 1.4
    /// §2.2.2.5.1.6-8) over this returned collection — `None` (a
    /// `#[cfg(test)]` fixture with no live reader) leaves
    /// `absolute_generation_rank == generation_rank` for every sample.
    fn from_state(
        samples: Vec<(UserSample, crate::entities::ReadSampleState, DcpsSampleInfo)>,
        tracker: Option<&zerodds_dcps::instance_tracker::InstanceTracker>,
    ) -> Box<Self> {
        let mut payloads: Vec<Vec<u8>> = Vec::with_capacity(samples.len());
        // (representation, big_endian) per sample, parallel to `dcps_infos`.
        let mut wire_repr: Vec<(u8, u8)> = Vec::with_capacity(samples.len());
        let mut dcps_infos: Vec<DcpsSampleInfo> = Vec::with_capacity(samples.len());

        for (s, state, mut info) in samples {
            info.sample_state = match state {
                crate::entities::ReadSampleState::Read => SampleStateKind::Read,
                crate::entities::ReadSampleState::NotRead => SampleStateKind::NotRead,
            };
            match s {
                UserSample::Alive {
                    payload,
                    representation,
                    big_endian,
                    ..
                } => {
                    payloads.push(payload.to_vec());
                    wire_repr.push((representation, u8::from(big_endian)));
                }
                UserSample::Lifecycle { .. } => {
                    payloads.push(Vec::new());
                    wire_repr.push((0, 0));
                }
                // The end-of-coherent-set marker (DDSI-RTPS 2.5 §8.7.6) carries
                // no user data and no instance; `resolve_instances_and_ownership`
                // already drops it before it reaches here — defensive.
                UserSample::CoherentSetEnd { .. } => continue,
            }
            dcps_infos.push(info);
        }

        // §2.2.2.5.1.6-8: sample_rank/generation_rank/absolute_generation_rank
        // are properties of THIS returned collection, not of a single baked
        // sample — computed once, here, over the whole batch. Not yet
        // called by the typed `DataReader`'s own `take_filtered`/
        // `read_filtered` (`crates/dcps/src/subscriber.rs`) — a pre-existing
        // gap in that crate, out of scope here (see this file's own doc
        // comment on `resolve_instances_and_ownership`); `compute_ranks`
        // lives in `zerodds_dcps::sample_info` precisely so that gap can be
        // closed with a two-line change, not a second implementation.
        zerodds_dcps::sample_info::compute_ranks(&mut dcps_infos, |handle| {
            tracker
                .and_then(|t| t.get_by_handle(handle))
                .map(|s| (s.disposed_generation_count, s.no_writers_generation_count))
                .unwrap_or((0, 0))
        });

        let mut infos: Vec<ZeroDdsSampleInfo> = Vec::with_capacity(dcps_infos.len());
        for (info, (representation, big_endian)) in dcps_infos.into_iter().zip(wire_repr) {
            let (source_timestamp_sec, source_timestamp_nanosec) =
                time_parts(info.source_timestamp);
            infos.push(ZeroDdsSampleInfo {
                sample_state: match info.sample_state {
                    SampleStateKind::Read => 1,
                    SampleStateKind::NotRead => 2,
                },
                view_state: match info.view_state {
                    ViewStateKind::New => 1,
                    ViewStateKind::NotNew => 2,
                },
                instance_state: match info.instance_state {
                    InstanceStateKind::Alive => 1,
                    InstanceStateKind::NotAliveDisposed => 2,
                    InstanceStateKind::NotAliveNoWriters => 4,
                },
                disposed_generation_count: info.disposed_generation_count,
                no_writers_generation_count: info.no_writers_generation_count,
                sample_rank: info.sample_rank,
                generation_rank: info.generation_rank,
                absolute_generation_rank: info.absolute_generation_rank,
                source_timestamp_sec,
                source_timestamp_nanosec,
                instance_handle: info.instance_handle.as_raw(),
                publication_handle: info.publication_handle.as_raw(),
                valid_data: info.valid_data,
                representation,
                big_endian,
            });
        }

        // Buffers + lengths in separate Vecs so that their pointers are stable
        // (Vec<Vec<u8>>::as_mut_ptr() of the inner Vecs is stable
        // as long as the outer Vec does not re-allocate).
        let mut buffers: Vec<*mut u8> = Vec::with_capacity(payloads.len());
        let mut lengths: Vec<usize> = Vec::with_capacity(payloads.len());
        for v in payloads.iter_mut() {
            buffers.push(v.as_mut_ptr());
            lengths.push(v.len());
        }
        Box::new(LoanMemory {
            payloads,
            buffers,
            lengths,
            infos,
        })
    }
}

pub(crate) fn u64_from_guid(g: [u8; 16]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in g.iter() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The KeyHash of the single instance of a keyless topic: sixteen zero bytes.
///
/// DDS-XTypes 1.3 §7.6.8 builds the KeyHash from the type's Key Holder — the
/// serialized key members. A type without key members has an empty Key Holder,
/// which is shorter than 16 bytes and is therefore zero-padded to the full
/// width, i.e. it is all zeros. `dcps::Subscriber::passes_exclusive_ownership`
/// uses the same value for the same reason.
const KEYLESS_KEY_HASH: [u8; 16] = [0u8; 16];

/// Last-resort per-instance KeyHash for an `Alive` sample on a **keyed**
/// topic, for the case `zerodds_dcps::runtime::DcpsRuntime` itself could
/// not supply one.
///
/// Since a keyed reader's `type_identifier` resolves, the runtime
/// computes the real KeyHash from the type (XTypes 1.3 §7.6.8) whenever a
/// writer's DATA leaves `PID_KEY_HASH` out of its inline QoS — Eclipse
/// Cyclone always does (DDSI-RTPS 2.5 §9.6.3.3 makes it optional) — and
/// `UserSample::Alive.key_hash` carries that hash already (a lifecycle
/// marker's `UserSample::Lifecycle.key_hash` is never `None`: the runtime
/// resolves it the same way or falls back to its own key-only-payload
/// reconstruction, never to this function). This function only still runs
/// when the runtime had no type to compute from (an untyped C-FFI reader
/// created without a registered `TypeObject`) or the sample's body did not
/// decode as the registered type; the runtime reports either case once per
/// (reader, writer) via the `reader.key_hash.unavailable` observability
/// event before leaving `key_hash` empty for this fallback. Distinct
/// payloads then become distinct instances — not a KeyHash in the sense of
/// XTypes 1.3 §7.6.8, only the untyped surface's fallback identity.
fn payload_key_hash(payload: &[u8]) -> [u8; 16] {
    let mut kh = [0u8; 16];
    // Two independent FNV-1a streams (different seeds) fill the 16-byte hash,
    // giving a low-collision instance identity for the untyped surface.
    let seeds: [u64; 2] = [0xcbf2_9ce4_8422_2325, 0x1000_0000_01b3_27d4];
    for (i, &seed) in seeds.iter().enumerate() {
        let mut h = seed;
        for &b in payload {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        kh[i * 8..i * 8 + 8].copy_from_slice(&h.to_le_bytes());
    }
    kh
}

/// The topic kind the reader endpoint itself declares: DDSI-RTPS 2.5 §9.3.1.2
/// Table 9.1 encodes `WITH_KEY` / `NO_KEY` in the `entityKind` octet of the
/// endpoint's `EntityId`, and [`EntityKind::topic_kind`] is that mapping.
/// `NO_KEY` is the RTPS statement that "the topic carries a single instance"
/// (§8.2.1.2 `TopicKind_t`).
///
/// An `EntityId` that carries no topic at all (a participant or group octet)
/// cannot reach this path — a DataReader is always built from
/// `EntityId::user_reader_with_key` / `user_reader_no_key`. `WITH_KEY` is the
/// conservative reading for such an octet: it keeps the per-payload instance
/// identity instead of collapsing samples of different instances onto one.
fn reader_topic_kind(drr: &ZeroDdsDataReader) -> TopicKind {
    drr.eid
        .entity_kind
        .topic_kind()
        .unwrap_or(TopicKind::WithKey)
}

/// Walks freshly pulled-off-the-channel `Alive`/`Lifecycle` samples and
/// bakes each one's [`DcpsSampleInfo`]: (1) applies the EXCLUSIVE-ownership
/// arbitration via the validated dcps core
/// [`InstanceTracker::should_accept_sample_under_exclusive_ownership`]
/// (Spec §2.2.3.23), (2) resolves the per-instance `InstanceHandle` for
/// each surviving sample (Spec §2.2.2.5.4) so dispose/unregister
/// transitions are distinguishable from C-FFI bindings, and (3) reads the
/// resulting view_state/instance_state/generation counts straight off the
/// [`InstanceTracker`] entry that (1)+(2) just mutated — the SAME
/// per-instance state machine the typed `DataReader`'s `ingest_into_cache`
/// (`crates/dcps/src/subscriber.rs`) reads and mutates, not a second one
/// built for the C-FFI.
///
/// Must be called EXACTLY ONCE per raw sample, at the point it is first
/// taken off the channel — see the `read_cache` field doc on
/// [`ZeroDdsDataReader`] for why re-baking an already-cached entry on a
/// later `read()`/`take()` call would corrupt this state. `sample_rank`/
/// `generation_rank`/`absolute_generation_rank` are deliberately NOT filled
/// here: they describe the returned collection as a whole, not a single
/// freshly-baked sample, and [`LoanMemory::from_state`] computes them once
/// per batch instead.
///
/// Keyed identities use the full KeyHash and an allocated non-NIL handle,
/// bounded by ResourceLimits.max_instances. A sample that would open an
/// instance beyond it is refused on its own and counted on the reader's
/// SAMPLE_REJECTED (RELIABLE) or SAMPLE_LOST (BEST_EFFORT) status, never
/// silently replaced with a truncated or payload key.
/// A keyless reader tracks the topic's single instance (Spec §2.2.1.2.2)
/// under `KEYLESS_KEY_HASH`, as the typed `DataReader` does, and reads its
/// handle and states off the tracker like a keyed one.
pub(crate) fn resolve_instances_and_ownership(
    drr: &ZeroDdsDataReader,
    collected: Vec<UserSample>,
) -> Result<Vec<(UserSample, DcpsSampleInfo)>, ZeroDdsStatus> {
    use zerodds_rtps::history_cache::ChangeKind;
    let exclusive = matches!(drr.ownership, zerodds_dcps::qos::OwnershipKind::Exclusive);
    let keyed = matches!(reader_topic_kind(drr), TopicKind::WithKey);
    let limit = {
        let qos = drr.qos.lock().map_err(|_| ZeroDdsStatus::Error)?;
        qos.resource_limits.max_instances
    };
    // DDS 1.4 §2.2.3.19: a sample that would open an instance beyond
    // `max_instances` is refused on its own — SAMPLE_REJECTED with
    // REJECTED_BY_INSTANCES_LIMIT, "A (received) sample has been rejected"
    // (§2.2.4.1), on a RELIABLE and on a BEST_EFFORT reader — and every
    // other sample of the batch is delivered, as the typed `DataReader`
    // does (`ingest_alive` in `crates/dcps/src/subscriber.rs`). The whole
    // batch used to fail with `OutOfResources`, which dropped the
    // admissible samples next to the refused one: a C++ reader with the
    // PSM's former 10-instance default lost data of instances it already
    // held. A BEST_EFFORT one counted it as SAMPLE_LOST, a sample "never
    // received".
    let refuse = || {
        drr.rt.record_sample_rejected(
            drr.eid,
            zerodds_dcps::status::SampleRejectedStatusKind::RejectedByInstancesLimit,
            InstanceHandle::from_raw(0),
        );
    };
    // A zero-initialised C QoS struct has `max_instances == 0`, which is
    // "unset", not "no instances": the QoS crate's own default is
    // `LENGTH_UNLIMITED` (negative, DDS 1.4 §2.2.3.6), so a non-positive
    // limit maps to the tracker's unbounded path and only a positive
    // count enforces the cap.
    let limit = if limit > 0 { limit } else { i32::MAX };
    let mut out = Vec::with_capacity(collected.len());
    for s in collected {
        match &s {
            UserSample::Alive {
                payload,
                key_hash,
                writer_guid,
                writer_strength,
                source_timestamp,
                ..
            } => {
                let kh = if keyed {
                    key_hash.unwrap_or_else(|| payload_key_hash(payload.as_ref()))
                } else {
                    KEYLESS_KEY_HASH
                };
                // DDS 1.4 §2.2.1.2.2: a topic without a key has one instance,
                // tracked under `KEYLESS_KEY_HASH`.
                let Some(handle) = drr.instances.ensure_registered_limited(kh, limit) else {
                    refuse();
                    continue;
                };
                if exclusive
                    && !drr
                        .instances
                        .should_accept_sample_under_exclusive_ownership(
                            &kh,
                            *writer_guid,
                            *writer_strength,
                        )
                {
                    // Weaker writer's sample on an owned instance → drop.
                    // `ensure_registered_limited` above already gave the
                    // owner-tracker its slot for this instance, so no
                    // separate `observe_sample` call is needed first.
                    continue;
                }
                let mut info = DcpsSampleInfo {
                    sample_state: SampleStateKind::NotRead,
                    source_timestamp: baked_source_timestamp(*source_timestamp),
                    publication_handle: InstanceHandle::from_raw(u64_from_guid(*writer_guid)),
                    valid_data: true,
                    ..DcpsSampleInfo::default()
                };
                // §2.2.2.5.1: the instance is written by this writer. When
                // the last writer of an instance goes away it becomes
                // NOT_ALIVE_NO_WRITERS, so the set is kept per instance
                // (mirrors `DataReader::ingest_into_cache`'s own
                // `note_writer` call).
                drr.instances.note_writer(handle, *writer_guid);
                info.instance_handle = handle;
                if let Some(state) = drr.instances.get_by_handle(handle) {
                    info.view_state = if state.reader_view_new {
                        ViewStateKind::New
                    } else {
                        ViewStateKind::NotNew
                    };
                    info.instance_state = state.kind;
                    info.disposed_generation_count = state.disposed_generation_count;
                    info.no_writers_generation_count = state.no_writers_generation_count;
                }
                out.push((s, info));
            }
            UserSample::Lifecycle {
                key_hash,
                kind,
                source_timestamp,
                ..
            } => {
                let mut info = DcpsSampleInfo {
                    sample_state: SampleStateKind::NotRead,
                    source_timestamp: baked_source_timestamp(*source_timestamp),
                    valid_data: false,
                    ..DcpsSampleInfo::default()
                };
                {
                    // Lifecycle markers carry the writer's real KeyHash (RTPS
                    // 2.5 §9.6.3.8); it names the same handle as the
                    // instance's alive samples through the same full-key
                    // registry. A topic without a key has its one instance
                    // whatever hash the marker carries.
                    let key_hash = if keyed { key_hash } else { &KEYLESS_KEY_HASH };
                    let Some(instance) = drr
                        .instances
                        .lookup(key_hash)
                        .or_else(|| drr.instances.ensure_registered_limited(*key_hash, limit))
                    else {
                        refuse();
                        continue;
                    };
                    match kind {
                        ChangeKind::NotAliveDisposed | ChangeKind::NotAliveDisposedUnregistered => {
                            drr.instances.dispose(instance, None);
                        }
                        ChangeKind::NotAliveUnregistered => {
                            drr.instances.unregister(instance, None);
                        }
                        _ => {}
                    }
                    info.instance_handle = instance;
                    if let Some(state) = drr.instances.get_by_handle(instance) {
                        info.view_state = if state.reader_view_new {
                            ViewStateKind::New
                        } else {
                            ViewStateKind::NotNew
                        };
                        info.instance_state = state.kind;
                        info.disposed_generation_count = state.disposed_generation_count;
                        info.no_writers_generation_count = state.no_writers_generation_count;
                    }
                }
                out.push((s, info));
            }
            // Not a sample and not an instance — see `loan_from_samples`.
            UserSample::CoherentSetEnd { .. } => continue,
        }
    }
    Ok(out)
}

/// Whether a raw wire item passes this reader's ContentFilteredTopic
/// filter (Spec §2.2.2.3.3), if any. A filter is defined over the topic's
/// data content, so `Lifecycle` markers always pass it; the
/// end-of-coherent-set marker never does — the C API holds no
/// coherent-set state of its own, so it is dropped outright rather than
/// forwarded unresolved.
pub(crate) fn cft_passes(s: &UserSample, filter: Option<&CftFilter>) -> bool {
    match s {
        UserSample::Alive {
            payload,
            representation,
            big_endian,
            ..
        } => filter.is_none_or(|f| f.accepts(payload.as_ref(), *representation, *big_endian)),
        UserSample::Lifecycle { .. } => true,
        UserSample::CoherentSetEnd { .. } => false,
    }
}

/// Pulls up to `max` items off the reader's channel, applies the CFT
/// filter, bakes each one's `SampleInfo` via
/// [`resolve_instances_and_ownership`], and appends the result to
/// `read_cache` as freshly-arrived (`NotRead`) entries. `max = usize::MAX`
/// drains everything currently queued — the typed `DataReader`'s own
/// `ingest_into_cache` has no per-call cap either; only the later
/// extraction step (`zerodds_dr_take`'s `max_samples`) bounds what a single
/// call returns to the caller. `zerodds_dr_take` passes a smaller `max`, so
/// a call resolves only the samples it is about to hand out.
///
/// # Errors
/// `Error` only when the reader's QoS lock is poisoned. A sample beyond
/// `ResourceLimits.max_instances` (Spec §2.2.3.19) is refused on its own
/// inside `resolve_instances_and_ownership`, and the rest of the batch is
/// cached.
pub(crate) fn drain_fresh_into_read_cache(
    drr: &ZeroDdsDataReader,
    max: usize,
) -> Result<(), ZeroDdsStatus> {
    // SAFETY: a reader's subscriber outlives it (`delete_subscriber`
    // refuses a subscriber with readers); NULL for none.
    let sb = unsafe { drr.subscriber.as_ref() };
    let presentation = ReaderPresentation::of(sb);
    // In the order it arrived: what the group order released or a sibling
    // pumped (`pump_group_siblings`), then the channel.
    let mut fresh: Vec<UserSample> = drr
        .group_inbox
        .lock()
        .map(|mut inbox| core::mem::take(&mut *inbox))
        .unwrap_or_default();
    if let Ok(rx) = drr.rx.lock() {
        // Under the group order the whole channel: a number another sample
        // waits for may be in it.
        let max = if presentation.group { usize::MAX } else { max };
        let mut pulled = 0;
        while pulled < max {
            match rx.try_recv() {
                Ok(s) => {
                    pulled += 1;
                    fresh.extend(admit(drr, sb, presentation, s));
                }
                Err(_) => break,
            }
        }
    }
    if let (Some(sb), true) = (sb, presentation.group) {
        // The siblings' channels first: a number this reader's samples wait
        // for may sit on one of them. What the release hands this reader is
        // in its inbox, behind what an earlier release put there.
        pump_group_siblings(drr, sb, presentation);
        sb.group.release(&drr.rt);
        if let Ok(mut inbox) = drr.group_inbox.lock() {
            fresh.append(&mut inbox);
        }
    }
    if let Some(filter) = &drr.cft_filter {
        fresh.retain(|s| cft_passes(s, Some(filter)));
    }
    let resolved = if fresh.is_empty() {
        Vec::new()
    } else {
        resolve_instances_and_ownership(drr, fresh)?
    };
    admit_into_cache(drr, resolved)
}

/// HISTORY and RESOURCE_LIMITS on the reader's cache (DDS 1.4 §2.2.3.18,
/// §2.2.3.19), as the typed `DataReader`'s `ingest_alive` applies them:
/// KEEP_LAST(depth) evicts the oldest sample of the instance; over
/// `max_samples_per_instance` or `max_samples` a BEST_EFFORT reader evicts
/// the oldest, a RELIABLE one refuses the sample — SAMPLE_REJECTED, once —
/// and holds it, with what came after it, for the next ingest. Samples
/// without data do not count. A non-positive limit of a zero-initialised
/// C QoS is no limit, a depth below 1 is 1. The reader applied none of
/// them but `max_instances`.
fn admit_into_cache(
    drr: &ZeroDdsDataReader,
    resolved: Vec<(UserSample, DcpsSampleInfo)>,
) -> Result<(), ZeroDdsStatus> {
    use zerodds_dcps::status::SampleRejectedStatusKind as Rejected;
    let (depth, max_samples, per_instance, reliable) = {
        let q = drr.qos.lock().map_err(|_| ZeroDdsStatus::Error)?;
        let limit = |n: i32| usize::try_from(n).ok().filter(|n| *n > 0);
        (
            matches!(q.history.kind, zerodds_qos::HistoryKind::KeepLast)
                .then(|| usize::try_from(q.history.depth.max(1)).unwrap_or(1)),
            limit(q.resource_limits.max_samples),
            limit(q.resource_limits.max_samples_per_instance),
            matches!(q.reliability.kind, ReliabilityKind::Reliable),
        )
    };
    let mut held = drr.held_back.lock().map_err(|_| ZeroDdsStatus::Error)?;
    if held.is_empty() && resolved.is_empty() {
        return Ok(());
    }
    let mut cache = drr.read_cache.lock().map_err(|_| ZeroDdsStatus::Error)?;
    let earlier = core::mem::take(&mut *held);
    let arriving = earlier
        .into_iter()
        .map(|entry| (true, entry))
        .chain(resolved.into_iter().map(|entry| (false, entry)));
    let mut refusing = false;
    for (counted, (sample, info)) in arriving {
        // Nothing overtakes a refused sample.
        if refusing {
            held.push((sample, info));
            continue;
        }
        if !info.valid_data {
            cache.push((sample, crate::entities::ReadSampleState::NotRead, info));
            continue;
        }
        let handle = info.instance_handle;
        let of_instance =
            |cache: &[(UserSample, crate::entities::ReadSampleState, DcpsSampleInfo)]| {
                cache
                    .iter()
                    .filter(|e| e.2.valid_data && e.2.instance_handle == handle)
                    .count()
            };
        if let Some(depth) = depth {
            while of_instance(&cache) >= depth && evict_oldest(&mut cache, Some(handle)) {}
        }
        let over = if per_instance.is_some_and(|n| of_instance(&cache) >= n) {
            Some((Rejected::RejectedBySamplesPerInstanceLimit, Some(handle)))
        } else if max_samples.is_some_and(|n| cache.iter().filter(|e| e.2.valid_data).count() >= n)
        {
            Some((Rejected::RejectedBySamplesLimit, None))
        } else {
            None
        };
        if let Some((kind, of)) = over {
            if reliable {
                if !counted {
                    drr.rt.record_sample_rejected(drr.eid, kind, handle);
                }
                held.push((sample, info));
                refusing = true;
                continue;
            }
            evict_oldest(&mut cache, of);
        }
        cache.push((sample, crate::entities::ReadSampleState::NotRead, info));
    }
    Ok(())
}

/// Removes the oldest sample with data of `instance` (of any instance for
/// `None`) from `cache`; `false` if there is none.
fn evict_oldest(
    cache: &mut Vec<(UserSample, crate::entities::ReadSampleState, DcpsSampleInfo)>,
    instance: Option<InstanceHandle>,
) -> bool {
    match cache
        .iter()
        .position(|e| e.2.valid_data && instance.is_none_or(|h| e.2.instance_handle == h))
    {
        Some(at) => {
            cache.remove(at);
            true
        }
        None => false,
    }
}

/// What the reader's Subscriber's PRESENTATION asks of the samples off its
/// channel and of a read or take (DDS 1.4 §2.2.3.6, §2.2.2.5.3.8), as the
/// typed `DataReader` reads it.
#[derive(Debug, Default, Clone, Copy)]
struct ReaderPresentation {
    /// `coherent_access`: a coherent set is presented all or nothing.
    coherent: bool,
    /// `access_scope` GROUP with ordered or coherent access: the samples of
    /// a GROUP-scope publisher are ordered across the readers (DDSI-RTPS
    /// 2.5 §8.7.5).
    group: bool,
    /// How a read or take presents its samples.
    order: ReadOrder,
}

/// The order a read or take returns its samples in (DDS 1.4 §2.2.2.5.3.8).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ReadOrder {
    /// The samples of an instance are consecutive (rules 1, 2, 4).
    #[default]
    Grouped,
    /// In the order they were written across instances (rule 3, TOPIC with
    /// `ordered_access`).
    InWriteOrder,
    /// At most one sample (rule 5, GROUP with `ordered_access`).
    OnePerRead,
}

impl ReaderPresentation {
    fn of(sb: Option<&ZeroDdsSubscriber>) -> Self {
        use zerodds_qos::PresentationAccessScope as Scope;
        sb.and_then(|sb| sb.qos.lock().ok())
            .map(|q| {
                let p = &q.presentation;
                Self {
                    coherent: p.coherent_access,
                    group: matches!(p.access_scope, Scope::Group)
                        && (p.ordered_access || p.coherent_access),
                    order: match (p.access_scope, p.ordered_access) {
                        (Scope::Group, true) => ReadOrder::OnePerRead,
                        (Scope::Topic, true) => ReadOrder::InWriteOrder,
                        _ => ReadOrder::Grouped,
                    },
                }
            })
            .unwrap_or_default()
    }

    /// The most samples one read or take returns: one under GROUP with
    /// `ordered_access`, else no limit.
    fn read_limit(&self) -> usize {
        match self.order {
            ReadOrder::OnePerRead => 1,
            _ => usize::MAX,
        }
    }

    /// Puts a batch in arrival order into the order [`Self::order`] asks
    /// for: "samples belonging to the same data-instance are consecutive",
    /// a stable sort by instance handle, unless the scope is TOPIC or GROUP
    /// with `ordered_access`. The C reader returned the instances
    /// interleaved.
    fn arrange(
        &self,
        batch: &mut [(UserSample, crate::entities::ReadSampleState, DcpsSampleInfo)],
    ) {
        if self.order != ReadOrder::Grouped {
            return;
        }
        let Some(first) = batch.first().map(|e| e.2.instance_handle) else {
            return;
        };
        if batch.iter().all(|e| e.2.instance_handle == first) {
            return;
        }
        batch.sort_by_key(|e| e.2.instance_handle.as_raw());
    }
}

/// One item off the channel of `drr`: into the Subscriber's group order
/// when it orders by group and the item is a GROUP-scope publisher's; else
/// through the reader's coherent-set gate. The items the reader may act on
/// now; never an end-of-coherent-set marker. An incomplete set is reported
/// and discarded.
fn admit(
    drr: &ZeroDdsDataReader,
    sb: Option<&ZeroDdsSubscriber>,
    presentation: ReaderPresentation,
    item: UserSample,
) -> Vec<UserSample> {
    let item = match sb {
        Some(sb) if presentation.group => match sb.group.order.offer(&drr.rt, drr.eid, item) {
            Some(item) => item,
            None => return Vec::new(),
        },
        _ => item,
    };
    drr.coherent_hold
        .gate(item, presentation.coherent, |members| {
            // SAFETY: a reader's topic outlives it.
            let topic = unsafe { drr.topic.as_ref() }.map_or("", |t| t.name.as_str());
            zerodds_dcps::presentation::report_incomplete_set(&drr.rt, topic, members);
        })
}

/// Hands what waits on the channels of the Subscriber's readers other than
/// `me` to the group order, the rest to their inboxes, as the typed
/// `Subscriber` pumps its readers.
fn pump_group_siblings(
    me: &ZeroDdsDataReader,
    sb: &ZeroDdsSubscriber,
    presentation: ReaderPresentation,
) {
    let readers: Vec<*mut ZeroDdsDataReader> = sb
        .datareaders
        .lock()
        .map(|list| list.clone())
        .unwrap_or_default();
    for other in readers {
        if ptr::eq(other, me) || other.is_null() {
            continue;
        }
        // SAFETY: the subscriber's readers are valid while it lists them.
        let other = unsafe { &*other };
        // A reader that drains its channel right now hands its samples to
        // the group order itself.
        let Ok(rx) = other.rx.try_lock() else {
            continue;
        };
        let mut keep = Vec::new();
        while let Ok(item) = rx.try_recv() {
            keep.extend(admit(other, Some(sb), presentation, item));
        }
        drop(rx);
        if !keep.is_empty() {
            if let Ok(mut inbox) = other.group_inbox.lock() {
                inbox.extend(keep);
            }
        }
    }
}

/// Pushes a synthetic `NOT_ALIVE_NO_WRITERS` marker into `read_cache` for
/// every instance that just lost its last matched writer (DDS 1.4
/// §2.2.2.5.1), mirroring the typed `DataReader`'s own
/// `apply_unmatched_writers` (`crates/dcps/src/subscriber.rs`): drains
/// `DcpsRuntime::take_reader_unmatched_writers` once, applies it to the
/// per-instance writer sets via `InstanceTracker::drop_writer`, and bakes
/// one marker per instance that is now writer-less. The marker then lives
/// in `read_cache` exactly like a channel sample — a non-consuming
/// `read()` sees it every time until a `take()` removes it.
///
/// A publisher that simply exits without unregistering anything is exactly
/// this case: no lifecycle marker is ever sent on the wire, and the state
/// change comes from the discovery/liveliness event alone.
///
/// Not covered by this crate's own tests: reproducing "a matched writer's
/// SEDP proxy actually goes away" needs a real unmatch event (remote
/// disconnect or liveliness lease expiry); a same-participant writer
/// deleted via `zerodds_pub_delete_datawriter` does not currently send one
/// (a separate, pre-existing gap in that function, not touched here). The
/// explicit `unregister_instance`/`dispose` path
/// (`resolve_instances_and_ownership`'s `Lifecycle` arm) exercises the same
/// `NOT_ALIVE_NO_WRITERS` reporting and IS covered.
fn apply_lost_writers(drr: &ZeroDdsDataReader) {
    let gone = drr.rt.take_reader_unmatched_writers(drr.eid);
    if gone.is_empty() {
        return;
    }
    let now = drr.rt.config.time_source.now();
    let mut markers = Vec::new();
    for writer in &gone {
        for handle in drr.instances.drop_writer(writer, Some(now)) {
            let Some(state) = drr.instances.get_by_handle(handle) else {
                continue; // should never happen — defensive.
            };
            let info = DcpsSampleInfo {
                sample_state: SampleStateKind::NotRead,
                view_state: if state.reader_view_new {
                    ViewStateKind::New
                } else {
                    ViewStateKind::NotNew
                },
                instance_state: state.kind,
                disposed_generation_count: state.disposed_generation_count,
                no_writers_generation_count: state.no_writers_generation_count,
                source_timestamp: now,
                instance_handle: handle,
                valid_data: false,
                ..DcpsSampleInfo::default()
            };
            // `info` (baked above) carries every field `LoanMemory::from_state`
            // reads; this placeholder only has to satisfy `UserSample`'s type
            // so the entry fits the same `read_cache` shape as a real sample.
            let placeholder = UserSample::Lifecycle {
                key_hash: KEYLESS_KEY_HASH,
                kind: zerodds_rtps::history_cache::ChangeKind::NotAliveUnregistered,
                key_holder: None,
                source_timestamp: None,
                group: None,
                writer_guid: [0; 16],
                coherent_set_sn: None,
                source_sequence_number: -1,
            };
            markers.push((placeholder, crate::entities::ReadSampleState::NotRead, info));
        }
    }
    if markers.is_empty() {
        return;
    }
    if let Ok(mut cache) = drr.read_cache.lock() {
        cache.extend(markers);
    }
}

/// Take: consumes samples from the cache + channel (Spec §2.2.2.5.3).
/// A sample beyond ResourceLimits.max_instances is refused on its own and
/// counted on the reader's status; the rest of the batch is delivered.
/// Goes through read_cache (sample-state READ + NOT_READ) and the channel
/// (sample-state NOT_READ), removes all delivered samples.
///
/// # Safety
/// `dr`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_take(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleArray,
    max_samples: usize,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: dr NULL-checked above; a reader's subscriber outlives it.
    if !group_access_allowed(unsafe { (*dr).subscriber.as_ref() }) {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    }
    // Masks that leave a state out take only the samples in them; the rest
    // stay (DDS 1.4 §2.2.2.5.3.8).
    let admits_all = |mask: u32, all: u32| mask == 0 || mask & all == all;
    if !(admits_all(sample_states, 0b11)
        && admits_all(view_states, 0b11)
        && admits_all(instance_states, 0b111))
    {
        // SAFETY: see fn # Safety doc — the same pointer contract.
        return unsafe {
            read_or_take_selected(
                dr,
                out,
                max_samples,
                Selection {
                    states: (sample_states, view_states, instance_states),
                    instance: InstanceSelect::Any,
                },
                true,
            )
        };
    }
    // SAFETY: dr NULL-checked above; a reader's subscriber outlives it.
    let presentation = ReaderPresentation::of(unsafe { (*dr).subscriber.as_ref() });
    let limit = if max_samples == 0 {
        usize::MAX
    } else {
        max_samples
    }
    .min(presentation.read_limit());

    // SAFETY: see fn # Safety doc — dr+out NULL-checked above; cache+rx live
    // for the reader.
    let mut collected = unsafe {
        let drr = &*dr;
        // 0) Apply any "last matched writer gone" transitions since the
        // previous call (Spec §2.2.2.5.1) — pushes already-baked markers
        // into read_cache, same as a channel sample.
        apply_lost_writers(drr);
        let mut collected: Vec<(UserSample, crate::entities::ReadSampleState, DcpsSampleInfo)> =
            Vec::new();
        // 1) Drain the read cache (already resolved — samples `read()` saw,
        // or a step-0 lost-writer marker), up to `limit`.
        if let Ok(mut cache) = drr.read_cache.lock() {
            while collected.len() < limit && !cache.is_empty() {
                collected.push(cache.remove(0));
            }
        }
        // 2) Fresh samples from the channel, bounded to what is still
        // missing from `limit` — resolved (baked) now, exactly once, then
        // drained right back out of read_cache. Kept bounded (not an
        // unconditional full drain) so a call resolves only the samples it
        // is about to hand out.
        let remaining = limit - collected.len();
        if remaining > 0 {
            if let Err(status) = drain_fresh_into_read_cache(drr, remaining) {
                return status as c_int;
            }
            if let Ok(mut cache) = drr.read_cache.lock() {
                while collected.len() < limit && !cache.is_empty() {
                    collected.push(cache.remove(0));
                }
            }
        }
        collected
    };
    if collected.is_empty() {
        // SAFETY: out NULL-checked at entry.
        unsafe {
            (*out).buffers = ptr::null_mut();
            (*out).lengths = ptr::null_mut();
            (*out).infos = ptr::null_mut();
            (*out).count = 0;
            (*out).loan_token = ptr::null_mut();
        }
        return ZeroDdsStatus::NoData as c_int;
    }
    // §2.2.2.5.1.2: once a sample is handed to the application, the
    // instance's view is no longer NEW for whatever arrives after it — does
    // not affect any `view_state` already baked into `collected` (a later
    // sample of the same instance in THIS SAME batch was baked before this
    // runs, so two samples of a newly-seen instance taken together both
    // legitimately report NEW, same as the typed `DataReader`).
    // SAFETY: dr NULL-checked at entry; the reader lives for this call.
    unsafe {
        let drr = &*dr;
        for (_, _, info) in &collected {
            if !info.instance_handle.is_nil() {
                drr.instances.mark_view_seen(info.instance_handle);
            }
        }
    }
    presentation.arrange(&mut collected);
    // SAFETY: dr NULL-checked at entry.
    finalize_loan(out, collected, Some(unsafe { &(*dr).instances }))
}

/// Read: non-destructive variant of take (Spec §2.2.2.5.3.8). Returns the
/// samples in the three state masks from the cache and the channel; they
/// remain in the read cache with sample-state = READ.
///
/// # Safety
/// `dr`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_read(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleArray,
    max_samples: usize,
    sample_states: u32,
    view_states: u32,
    instance_states: u32,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: see fn # Safety doc — the same pointer contract.
    unsafe {
        read_or_take_selected(
            dr,
            out,
            max_samples,
            Selection {
                states: (sample_states, view_states, instance_states),
                instance: InstanceSelect::Any,
            },
            false,
        )
    }
}

/// Which instance a read or take selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstanceSelect {
    /// Every instance (`read`, `take`, `*_w_condition`).
    Any,
    /// The instance of this handle (§2.2.2.5.3.14 `read_instance`).
    Is(u64),
    /// The instance with the smallest handle above this one that has a
    /// sample in the states (§2.2.2.5.3.16 `read_next_instance`).
    After(u64),
}

/// The samples a read or take selects: its sample, view and instance state
/// masks (0 lets every state through, as everywhere in this API) and its
/// instance (DDS 1.4 §2.2.2.5.3.8 and the variants after it).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Selection {
    pub states: (u32, u32, u32),
    pub instance: InstanceSelect,
}

/// The sample, view and instance state bits of a cached sample, with the
/// values of the IDL PSM (DDS 1.4 §2.3.3).
pub(crate) fn entry_state_bits(
    state: crate::entities::ReadSampleState,
    info: &DcpsSampleInfo,
) -> (u32, u32, u32) {
    let s = match state {
        crate::entities::ReadSampleState::Read => 1u32,
        crate::entities::ReadSampleState::NotRead => 2u32,
    };
    let v = match info.view_state {
        ViewStateKind::New => 1u32,
        ViewStateKind::NotNew => 2u32,
    };
    let i = match info.instance_state {
        InstanceStateKind::Alive => 1u32,
        InstanceStateKind::NotAliveDisposed => 2u32,
        InstanceStateKind::NotAliveNoWriters => 4u32,
    };
    (s, v, i)
}

/// Whether a cached sample is in the state masks `(s, v, i)`.
pub(crate) fn entry_in_states(
    state: crate::entities::ReadSampleState,
    info: &DcpsSampleInfo,
    (s, v, i): (u32, u32, u32),
) -> bool {
    let (sb, vb, ib) = entry_state_bits(state, info);
    (s == 0 || s & sb != 0) && (v == 0 || v & vb != 0) && (i == 0 || i & ib != 0)
}

/// A read or take of what `select` names, at most `max_samples` (0: no
/// limit). What it does not select stays in the reader, in its state.
///
/// The state masks of `zerodds_dr_read` and `zerodds_dr_take` were not
/// looked at, and `read_instance`, `take_instance`, `*_next_instance` and
/// `*_w_condition` read or took every sample and filtered the result: a
/// take handed out one instance and dropped the samples of the others, a
/// read marked them READ.
///
/// # Safety
/// `dr` and `out` valid or null.
pub(crate) unsafe fn read_or_take_selected(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleArray,
    max_samples: usize,
    select: Selection,
    take: bool,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { read_or_take_selected_where(dr, out, max_samples, select, None, take) }
}

/// [`read_or_take_selected`] that also requires each sample to satisfy
/// `query` (a QueryCondition's expression, DDS 1.4 §2.2.2.5.9); a lifecycle
/// notice carries no data and passes.
///
/// # Safety
/// `dr` and `out` valid or null.
pub(crate) unsafe fn read_or_take_selected_where(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleArray,
    max_samples: usize,
    select: Selection,
    query: Option<&crate::condition_ffi::Query>,
    take: bool,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: dr NULL-checked above; a reader's subscriber outlives it.
    let presentation = ReaderPresentation::of(unsafe { (*dr).subscriber.as_ref() });
    let limit = if max_samples == 0 {
        usize::MAX
    } else {
        max_samples
    }
    .min(presentation.read_limit());
    // SAFETY: dr NULL-checked above; the reader lives for this call.
    let drr = unsafe { &*dr };
    // SAFETY: a reader's subscriber outlives it.
    if !group_access_allowed(unsafe { drr.subscriber.as_ref() }) {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    }
    apply_lost_writers(drr);
    if let Err(status) = drain_fresh_into_read_cache(drr, usize::MAX) {
        return status as c_int;
    }
    let mut collected = {
        let Ok(mut cache) = drr.read_cache.lock() else {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        };
        // A query that cannot be evaluated is an error as soon as there is a sample it would
        // have to judge; with none, there is simply no data.
        if query.is_some_and(crate::condition_ffi::Query::blocked)
            && cache.iter().any(|e| {
                matches!(e.0, UserSample::Alive { .. }) && entry_in_states(e.1, &e.2, select.states)
            })
        {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        let in_states = |e: &(UserSample, crate::entities::ReadSampleState, DcpsSampleInfo)| {
            entry_in_states(e.1, &e.2, select.states) && query.is_none_or(|q| q.matches(&e.0))
        };
        // `None`: every instance; `Some(None)`: no instance qualifies.
        let instance: Option<Option<u64>> = match select.instance {
            InstanceSelect::Any => None,
            InstanceSelect::Is(h) => Some(Some(h)),
            InstanceSelect::After(previous) => Some(
                cache
                    .iter()
                    .filter(|e| in_states(e))
                    .map(|e| e.2.instance_handle.as_raw())
                    .filter(|h| *h != 0 && *h > previous)
                    .min(),
            ),
        };
        let selected = |e: &(UserSample, crate::entities::ReadSampleState, DcpsSampleInfo)| {
            in_states(e)
                && match instance {
                    None => true,
                    Some(wanted) => wanted == Some(e.2.instance_handle.as_raw()),
                }
        };
        let mut collected = Vec::new();
        if let Some(ordered) = query.filter(|q| q.orders()) {
            // `ORDER BY` (Annex B.2): the selected samples sorted by the
            // fields, the first `limit` of them returned.
            let mut chosen: Vec<usize> =
                (0..cache.len()).filter(|i| selected(&cache[*i])).collect();
            let keys: alloc::collections::BTreeMap<usize, Vec<zerodds_sql_filter::Value>> = chosen
                .iter()
                .map(|i| (*i, ordered.order_key(&cache[*i].0)))
                .collect();
            chosen.sort_by(|a, b| {
                for (left, right) in keys[a].iter().zip(keys[b].iter()) {
                    match left.order(right) {
                        core::cmp::Ordering::Equal => {}
                        unequal => return unequal,
                    }
                }
                core::cmp::Ordering::Equal
            });
            chosen.truncate(limit);
            if take {
                let mut slots: Vec<Option<_>> = cache.drain(..).map(Some).collect();
                for i in &chosen {
                    if let Some(entry) = slots[*i].take() {
                        collected.push(entry);
                    }
                }
                *cache = slots.into_iter().flatten().collect();
            } else {
                for i in &chosen {
                    collected.push(cache[*i].clone());
                    cache[*i].1 = crate::entities::ReadSampleState::Read;
                }
            }
        } else if take {
            let mut at = 0;
            while at < cache.len() && collected.len() < limit {
                if selected(&cache[at]) {
                    collected.push(cache.remove(at));
                } else {
                    at += 1;
                }
            }
        } else {
            for entry in cache.iter_mut() {
                if collected.len() >= limit {
                    break;
                }
                if selected(entry) {
                    collected.push(entry.clone());
                    entry.1 = crate::entities::ReadSampleState::Read;
                }
            }
        }
        collected
    };
    if collected.is_empty() {
        // SAFETY: out NULL-checked at entry.
        unsafe {
            (*out).buffers = ptr::null_mut();
            (*out).lengths = ptr::null_mut();
            (*out).infos = ptr::null_mut();
            (*out).count = 0;
            (*out).loan_token = ptr::null_mut();
        }
        return ZeroDdsStatus::NoData as c_int;
    }
    // §2.2.2.5.1.2 — see `zerodds_dr_take`.
    for (_, _, info) in &collected {
        if !info.instance_handle.is_nil() {
            drr.instances.mark_view_seen(info.instance_handle);
        }
    }
    // An `ORDER BY` fixes the order; otherwise the presentation does.
    if !query.is_some_and(|q| q.orders()) {
        presentation.arrange(&mut collected);
    }
    finalize_loan(out, collected, Some(&drr.instances))
}

fn finalize_loan(
    out: *mut ZeroDdsSampleArray,
    collected: Vec<(UserSample, crate::entities::ReadSampleState, DcpsSampleInfo)>,
    tracker: Option<&zerodds_dcps::instance_tracker::InstanceTracker>,
) -> c_int {
    let mut loan = LoanMemory::from_state(collected, tracker);
    let buffers_ptr = loan.buffers.as_mut_ptr();
    let lengths_ptr = loan.lengths.as_mut_ptr();
    let infos_ptr = loan.infos.as_mut_ptr();
    let count = loan.payloads.len();
    let token = Box::into_raw(loan) as *mut core::ffi::c_void;
    // SAFETY: out was NULL-checked by dr_take/dr_read before the call; the caller pledge
    // guarantees a valid ZeroDdsSampleArray struct.
    unsafe {
        (*out).buffers = buffers_ptr;
        (*out).lengths = lengths_ptr;
        (*out).infos = infos_ptr;
        (*out).count = count;
        (*out).loan_token = token;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Take next single sample.
///
/// # Safety
/// `dr`, `out_buf`, `out_len`, `out_info` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_take_next_sample(
    dr: *mut ZeroDdsDataReader,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    if dr.is_null() || out_buf.is_null() || out_len.is_null() || out_info.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // Delegates to `zerodds_dr_take` (max_samples=1) rather than a second,
    // unresolved single-sample path: that path used to build its
    // `SampleInfo` straight off the raw `UserSample` with no
    // `resolve_instances_and_ownership` call at all, so every field this
    // module now bakes from the `InstanceTracker` (view_state,
    // instance_state, generation counts, a real `instance_handle`) was
    // simply skipped for this entry point.
    let mut arr = ZeroDdsSampleArray {
        buffers: ptr::null_mut(),
        lengths: ptr::null_mut(),
        infos: ptr::null_mut(),
        count: 0,
        loan_token: ptr::null_mut(),
    };
    // SAFETY: dr NULL-checked above; `arr` is a valid stack-local out-param.
    let rc = unsafe { zerodds_dr_take(dr, &mut arr, 1, 0, 0, 0) };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    if arr.count == 0 {
        return ZeroDdsStatus::NoData as c_int;
    }
    // SAFETY: `zerodds_dr_take` returned Ok with count>=1, so
    // buffers/lengths/infos[0] are initialized; out_* NULL-checked above.
    unsafe {
        *out_buf = *arr.buffers;
        *out_len = *arr.lengths;
        *out_info = *arr.infos;
    }
    // Single-sample take: the caller has no `loan_token` to return, so the
    // loan behind `arr` is never freed via `zerodds_dr_return_loan` — the
    // pre-existing, documented RC1 leak-tolerance this entry point already
    // had (the payload buffer itself must stay valid for the caller, since
    // it was handed out through `out_buf` above).
    let _ = arr.loan_token;
    ZeroDdsStatus::Ok as c_int
}

/// Read next single sample. Identical to take_next_sample in RC1.
///
/// # Safety
/// Like `take_next_sample`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_read_next_sample(
    dr: *mut ZeroDdsDataReader,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { zerodds_dr_take_next_sample(dr, out_buf, out_len, out_info) }
}

/// Returns loaned sample buffers.
///
/// # Safety
/// `arr` must come from a previous `zerodds_dr_take`/`read`,
/// `loan_token` must still be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_return_loan(
    _dr: *mut ZeroDdsDataReader,
    arr: *mut ZeroDdsSampleArray,
) -> c_int {
    if arr.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — arr NULL-checked above; loan_token from
    // LoanMemory::new Box::into_raw in dr_take/read.
    unsafe {
        let token = (*arr).loan_token;
        if !token.is_null() {
            let _ = Box::from_raw(token as *mut LoanMemory);
        }
        (*arr).buffers = ptr::null_mut();
        (*arr).lengths = ptr::null_mut();
        (*arr).infos = ptr::null_mut();
        (*arr).count = 0;
        (*arr).loan_token = ptr::null_mut();
    }
    ZeroDdsStatus::Ok as c_int
}

/// Waits until `min` matched publications or a timeout.
///
/// # Safety
/// `dr` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_wait_for_matched(
    dr: *mut ZeroDdsDataReader,
    min: i32,
    timeout_ms: u64,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dr) } {
        return rc;
    }
    if dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — dr NULL-checked above.
    let (rt, eid) = unsafe { ((*dr).rt.clone(), (*dr).eid) };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let n = rt.user_reader_matched_count(eid) as i32;
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
// DataReader Statuses (6x)
// ---------------------------------------------------------------------------

/// LivelinessChangedStatus (Spec §2.2.4.1).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsLivelinessChangedStatus {
    /// Total alive count.
    pub alive_count: i32,
    /// Total not-alive count.
    pub not_alive_count: i32,
    /// Change in alive_count since last read.
    pub alive_count_change: i32,
    /// Change in not_alive_count since last read.
    pub not_alive_count_change: i32,
    /// Last writer that triggered a change.
    pub last_publication_handle: u64,
}

/// SubscriptionMatchedStatus.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsSubscriptionMatchedStatus {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Currently matched.
    pub current_count: i32,
    /// Change in current_count.
    pub current_count_change: i32,
    /// Last DataWriter that matched.
    pub last_publication_handle: u64,
}

/// RequestedDeadlineMissedStatus.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsRequestedDeadlineMissedStatus {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Last instance handle.
    pub last_instance_handle: u64,
}

/// RequestedIncompatibleQosStatus.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsRequestedIncompatibleQosStatus {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Last policy id.
    pub last_policy_id: u32,
}

/// SampleLostStatus.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsSampleLostStatus {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// SampleRejectedStatus.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsSampleRejectedStatus {
    /// Total cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Reason kind: 0=NotRejected, 1=ByInstancesLimit, 2=BySamplesLimit,
    /// 3=BySamplesPerInstanceLimit.
    pub last_reason: u32,
    /// Last instance handle.
    pub last_instance_handle: u64,
}

/// `LIVELINESS_CHANGED_STATUS`.
///
/// # Safety
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_liveliness_changed_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsLivelinessChangedStatus,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle; the status of a disabled
    // entity is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dr, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        // Reading the status resets its change fields (DDS 1.4 §2.2.4.1).
        let st = drr
            .rt
            .take_user_reader_liveliness_changed(drr.eid)
            .unwrap_or_default();
        *out = ZeroDdsLivelinessChangedStatus {
            alive_count: st.alive_count,
            not_alive_count: st.not_alive_count,
            alive_count_change: st.alive_count_change,
            not_alive_count_change: st.not_alive_count_change,
            last_publication_handle: st.last_publication_handle.as_raw(),
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_subscription_matched_status` (DDS 1.4 §2.2.2.5.3); as
/// `zerodds_dw_get_publication_matched_status`.
///
/// # Safety
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_subscription_matched_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSubscriptionMatchedStatus,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle; the status of a disabled
    // entity is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dr, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let st = drr
            .rt
            .take_user_reader_subscription_matched_status(drr.eid)
            .unwrap_or_default();
        *out = ZeroDdsSubscriptionMatchedStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
            current_count: st.current_count,
            current_count_change: st.current_count_change,
            last_publication_handle: st.last_publication_handle.as_raw(),
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_requested_deadline_missed_status` (DDS 1.4 §2.2.2.5.3). Reading
/// resets the change field.
///
/// # Safety
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_requested_deadline_missed_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsRequestedDeadlineMissedStatus,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle; the status of a disabled
    // entity is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dr, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let st = drr
            .rt
            .take_user_reader_requested_deadline_missed_status(drr.eid)
            .unwrap_or_default();
        *out = ZeroDdsRequestedDeadlineMissedStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
            // The runtime knows the instance by key hash; the reader's
            // tracker holds its handle.
            last_instance_handle: drr
                .rt
                .user_reader_missed_deadline_instance(drr.eid)
                .and_then(|key| drr.instances.lookup(&key))
                .unwrap_or(st.last_instance_handle)
                .as_raw(),
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_requested_incompatible_qos_status` (DDS 1.4 §2.2.2.5.3). Reading
/// resets the change field.
///
/// # Safety
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_requested_incompatible_qos_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsRequestedIncompatibleQosStatus,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle; the status of a disabled
    // entity is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dr, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let st = drr
            .rt
            .take_user_reader_requested_incompatible_qos_status(drr.eid)
            .unwrap_or_default();
        *out = ZeroDdsRequestedIncompatibleQosStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
            last_policy_id: st.last_policy_id,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_sample_lost_status` (DDS 1.4 §2.2.2.5.3). Reading resets the
/// change field.
///
/// # Safety
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_sample_lost_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleLostStatus,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle; the status of a disabled
    // entity is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dr, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let st = drr
            .rt
            .take_user_reader_sample_lost_status(drr.eid)
            .unwrap_or_default();
        *out = ZeroDdsSampleLostStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_sample_rejected_status` (DDS 1.4 §2.2.2.5.3). Reading resets the
/// change field.
///
/// # Safety
/// `dr` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_sample_rejected_status(
    dr: *mut ZeroDdsDataReader,
    out: *mut ZeroDdsSampleRejectedStatus,
) -> c_int {
    // SAFETY: a non-null `dr` is a live DataReader handle; the status of a disabled
    // entity is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dr, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    use zerodds_dcps::status::SampleRejectedStatusKind;
    // SAFETY: see fn # Safety doc — dr+out NULL-checked above.
    unsafe {
        let drr = &*dr;
        let st = drr
            .rt
            .take_user_reader_sample_rejected_status(drr.eid)
            .unwrap_or_default();
        let reason: u32 = match st.last_reason {
            SampleRejectedStatusKind::NotRejected => 0,
            SampleRejectedStatusKind::RejectedByInstancesLimit => 1,
            SampleRejectedStatusKind::RejectedBySamplesLimit => 2,
            SampleRejectedStatusKind::RejectedBySamplesPerInstanceLimit => 3,
        };
        *out = ZeroDdsSampleRejectedStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
            last_reason: reason,
            last_instance_handle: st.last_instance_handle.as_raw(),
        };
    }
    ZeroDdsStatus::Ok as c_int
}

// suppress unused-import warning when DataReaderQos is only used in entities.rs
#[allow(dead_code)]
fn _suppress(_: DataReaderQos, _: mpsc::Receiver<UserSample>) {}

/// A reader's PARTITION, PRESENTATION and GROUP_DATA — the three policies
/// DDS 1.4 places on the Subscriber (§2.2.3.13, §2.2.3.6, §2.2.3.3) and not
/// on the DataReader.
pub(crate) struct SubscriberScoped {
    pub(crate) partition: Vec<String>,
    pub(crate) presentation: zerodds_qos::PresentationQosPolicy,
    pub(crate) group_data: Vec<u8>,
}

/// The Subscriber's PARTITION, PRESENTATION and GROUP_DATA for a reader it
/// creates; a partition or group data the caller filled into the C ABI's
/// `ZeroDdsDataReaderQos` wins, as on the writer side (`publisher_ffi::
/// publisher_scoped`).
/// TIME_BASED_FILTER's minimum separation in nanoseconds for
/// `DcpsRuntime::set_user_reader_time_based_filter`; `0` (no filter) for
/// zero and for INFINITE.
pub(crate) fn time_based_filter_nanos(qos: &DataReaderQos) -> u128 {
    let separation = qos.time_based_filter.minimum_separation;
    if separation.is_infinite() {
        0
    } else {
        separation.to_nanos()
    }
}

pub(crate) fn subscriber_scoped(sb: &ZeroDdsSubscriber, qos: &DataReaderQos) -> SubscriberScoped {
    let subscriber = sb.qos.lock().map(|g| g.clone()).unwrap_or_default();
    SubscriberScoped {
        partition: if qos.partition.names.is_empty() {
            subscriber.partition.names
        } else {
            qos.partition.names.clone()
        },
        presentation: subscriber.presentation,
        group_data: if qos.group_data.value.is_empty() {
            subscriber.group_data.value
        } else {
            qos.group_data.value.clone()
        },
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::factory_ffi::{
        zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
    };
    use crate::participant_ffi::{
        zerodds_dp_create_subscriber, zerodds_dp_create_topic, zerodds_dp_delete_contained_entities,
    };

    fn mk(
        domain: u32,
    ) -> (
        *mut ZeroDdsDomainParticipant,
        *mut ZeroDdsSubscriber,
        *mut ZeroDdsTopic,
    ) {
        let f = zerodds_dpf_get_instance();
        let n = c"SubT";
        let tn = c"SubTy";
        // SAFETY: f from dpf_get_instance, n+tn statically valid.
        unsafe {
            let p = zerodds_dpf_create_participant(f, domain, ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            (p, sub, t)
        }
    }
    fn cleanup(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: p from mk; f static.
        unsafe {
            zerodds_dp_delete_contained_entities(p);
            zerodds_dpf_delete_participant(f, p);
        }
    }

    #[test]
    fn create_delete_datareader() {
        let (p, sub, t) = mk(51);
        // SAFETY: sub+t from mk, valid until cleanup.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert!(!dr.is_null());
            let rc = zerodds_sub_delete_datareader(sub, dr);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        }
        cleanup(p);
    }

    #[test]
    fn lookup_datareader_finds_existing() {
        let (p, sub, t) = mk(52);
        let n = c"SubT";
        // SAFETY: sub+t from mk; n static.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let f = zerodds_sub_lookup_datareader(sub, n.as_ptr());
            assert_eq!(f, dr);
        }
        cleanup(p);
    }

    /// `zerodds_sub_get_datareaders` writes `min(len, cap)` handles but
    /// reports the total and returns `OutOfResources` when the buffer is too
    /// small, so no reader is dropped silently
    /// (`docs/specs/zerodds-c-api-1.0.md` §2.8).
    #[test]
    fn sub_get_datareaders_reports_the_total() {
        let (p, sub, t) = mk(53);
        // SAFETY: sub+t from mk; every reader comes from a create call.
        unsafe {
            let drs: [*mut ZeroDdsDataReader; 3] = [
                zerodds_sub_create_datareader(sub, t, ptr::null()),
                zerodds_sub_create_datareader(sub, t, ptr::null()),
                zerodds_sub_create_datareader(sub, t, ptr::null()),
            ];
            assert!(drs.iter().all(|d| !d.is_null()));
            // §2.2.2.5.2.10: a reader is listed when it holds samples.
            for d in drs {
                assert!((*d).rt.test_inject_user_alive((*d).eid, alloc::vec![1u8]));
            }

            let mut count: usize = 0;

            // Too small: `cap` entries written, the total reported.
            let mut buf: [*mut ZeroDdsDataReader; 2] = [ptr::null_mut(); 2];
            let rc = zerodds_sub_get_datareaders(sub, buf.as_mut_ptr(), &mut count, 2, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::OutOfResources as c_int);
            assert_eq!(count, 3);
            assert_eq!(buf[0], drs[0]);
            assert_eq!(buf[1], drs[1]);

            // `cap == 0` with NULL asks for the length alone.
            let rc = zerodds_sub_get_datareaders(sub, ptr::null_mut(), &mut count, 0, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::OutOfResources as c_int);
            assert_eq!(count, 3);

            // Enough room: every entry, `Ok`.
            let mut all: [*mut ZeroDdsDataReader; 4] = [ptr::null_mut(); 4];
            let rc = zerodds_sub_get_datareaders(sub, all.as_mut_ptr(), &mut count, 4, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 3);
            assert_eq!(all[0], drs[0]);
            assert_eq!(all[1], drs[1]);
            assert_eq!(all[2], drs[2]);

            // NULL with `cap > 0` is refused.
            let rc = zerodds_sub_get_datareaders(sub, ptr::null_mut(), &mut count, 4, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::BadParameter as c_int);
        }
        cleanup(p);
    }

    /// DDS 1.4 §2.2.2.5.2.10: "returns the DataReader objects that contain
    /// samples with the specified sample_states, view_states, and
    /// instance_states" — a reader without samples, or with samples in
    /// other states, is not listed. It listed every reader.
    #[test]
    fn get_datareaders_lists_the_readers_with_samples_in_the_states() {
        let (p, sub, t) = mk(87);
        // SAFETY: sub+t from mk; the readers come from create calls.
        unsafe {
            let empty = zerodds_sub_create_datareader(sub, t, ptr::null());
            let full = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert!(!empty.is_null() && !full.is_null());
            assert!(
                (*full)
                    .rt
                    .test_inject_user_alive((*full).eid, alloc::vec![1u8])
            );
            let listed = |sample_states: u32| {
                let mut buf: [*mut ZeroDdsDataReader; 2] = [ptr::null_mut(); 2];
                let mut count = 0usize;
                let rc = zerodds_sub_get_datareaders(
                    sub,
                    buf.as_mut_ptr(),
                    &mut count,
                    2,
                    sample_states,
                    0,
                    0,
                );
                assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
                buf[..count].to_vec()
            };
            const READ: u32 = 1;
            const NOT_READ: u32 = 2;
            assert_eq!(listed(0), alloc::vec![full]);
            assert_eq!(listed(NOT_READ), alloc::vec![full]);
            assert!(listed(READ).is_empty());
            // A read makes the sample READ.
            let mut arr = empty_arr();
            assert_eq!(
                zerodds_dr_read(full, &mut arr, 0, 0, 0, 0),
                ZeroDdsStatus::Ok as c_int
            );
            zerodds_dr_return_loan(full, &mut arr);
            assert_eq!(listed(READ), alloc::vec![full]);
            assert!(listed(NOT_READ).is_empty());
        }
        cleanup(p);
    }

    #[test]
    fn from_state_fills_representation_and_byte_order() {
        // A loan-batch sample carries its wire representation + byte order out
        // to the FFI consumer (TS/C++) so it can dispatch decode vs decode_be.
        use zerodds_dcps::sample_bytes::SampleBytes;
        let alive = UserSample::Alive {
            payload: SampleBytes::from_vec(vec![1, 2, 3, 4]),
            key_hash: None,
            writer_guid: [0xAB; 16],
            writer_strength: 0,
            representation: 1, // XCDR2
            big_endian: true,  // big-endian peer
            source_timestamp: None,
            reception_timestamp: None,
            source_sequence_number: -1,
            coherent_set_sn: None,
            // Test fixture: no LIFESPAN — `0` == INFINITE.
            lifespan_nanos: 0,
            related_sample_identity: None,
            group: None,
            crypto_content: false,
        };
        let lifecycle = UserSample::Lifecycle {
            key_hash: [0; 16],
            kind: zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed,
            key_holder: None,
            source_timestamp: None,
            group: None,
            writer_guid: [0; 16],
            coherent_set_sn: None,
            source_sequence_number: -1,
        };
        let mem = LoanMemory::from_state(
            vec![
                (
                    alive,
                    crate::entities::ReadSampleState::NotRead,
                    DcpsSampleInfo {
                        instance_handle: InstanceHandle::from_raw(7),
                        valid_data: true,
                        ..DcpsSampleInfo::default()
                    },
                ),
                (
                    lifecycle,
                    crate::entities::ReadSampleState::NotRead,
                    DcpsSampleInfo {
                        instance_handle: InstanceHandle::from_raw(8),
                        valid_data: false,
                        ..DcpsSampleInfo::default()
                    },
                ),
            ],
            None,
        );
        assert_eq!(mem.infos.len(), 2);
        // Alive sample reflects the wire encap.
        assert!(mem.infos[0].valid_data);
        assert_eq!(mem.infos[0].representation, 1);
        assert_eq!(mem.infos[0].big_endian, 1);
        // Lifecycle marker has no payload → neutral 0/0.
        assert!(!mem.infos[1].valid_data);
        assert_eq!(mem.infos[1].representation, 0);
        assert_eq!(mem.infos[1].big_endian, 0);
    }

    #[test]
    fn take_on_empty_returns_no_data() {
        let (p, sub, t) = mk(53);
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 0,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: sub+t from mk; arr lives for the test.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let rc = zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
        }
        assert_eq!(arr.count, 0);
        cleanup(p);
    }

    #[test]
    fn return_loan_clears_array() {
        let (p, sub, t) = mk(54);
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 7,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: sub+t from mk; arr lives for the test.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let rc = zerodds_dr_return_loan(dr, &mut arr);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        }
        assert_eq!(arr.count, 0);
        cleanup(p);
    }

    #[test]
    fn statuses_default_ok() {
        let (p, sub, t) = mk(55);
        let mut a = ZeroDdsLivelinessChangedStatus::default();
        let mut b = ZeroDdsSubscriptionMatchedStatus::default();
        let mut c = ZeroDdsRequestedDeadlineMissedStatus::default();
        let mut d = ZeroDdsRequestedIncompatibleQosStatus::default();
        let mut e = ZeroDdsSampleLostStatus::default();
        let mut g = ZeroDdsSampleRejectedStatus::default();
        // SAFETY: sub+t from mk; status slots on the stack.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let ok = ZeroDdsStatus::Ok as c_int;
            assert_eq!(zerodds_dr_get_liveliness_changed_status(dr, &mut a), ok);
            assert_eq!(zerodds_dr_get_subscription_matched_status(dr, &mut b), ok);
            assert_eq!(
                zerodds_dr_get_requested_deadline_missed_status(dr, &mut c),
                ok
            );
            assert_eq!(
                zerodds_dr_get_requested_incompatible_qos_status(dr, &mut d),
                ok
            );
            assert_eq!(zerodds_dr_get_sample_lost_status(dr, &mut e), ok);
            assert_eq!(zerodds_dr_get_sample_rejected_status(dr, &mut g), ok);
        }
        cleanup(p);
    }

    /// `zerodds_dr_get_liveliness_changed_status` reports the changes since
    /// the last read and the writer that made them, and resets the changes
    /// (DDS 1.4 §2.2.4.1). It used to return the counts with both change
    /// fields and the handle fixed at 0.
    #[test]
    fn liveliness_changed_status_reports_and_resets_the_changes() {
        use crate::participant_ffi::zerodds_dp_create_publisher;
        use crate::publisher_ffi::zerodds_pub_create_datawriter;
        let domain = u32::try_from(zerodds_test_domains::unique_domain()).expect("domain");
        let (p, sub, t) = mk(domain);
        let mut first = ZeroDdsLivelinessChangedStatus::default();
        let mut second = ZeroDdsLivelinessChangedStatus::default();
        // SAFETY: p, sub and t from mk; the status slots live on the stack.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let publisher = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(publisher, t, ptr::null());
            assert!(!dr.is_null() && !dw.is_null());
            let ok = ZeroDdsStatus::Ok as c_int;
            assert_eq!(zerodds_dr_get_liveliness_changed_status(dr, &mut first), ok);
            assert_eq!(
                zerodds_dr_get_liveliness_changed_status(dr, &mut second),
                ok
            );
        }
        // The default LIVELINESS is AUTOMATIC: alive from its match.
        assert_eq!((first.alive_count, first.not_alive_count), (1, 0));
        assert_eq!(
            (first.alive_count_change, first.not_alive_count_change),
            (1, 0)
        );
        assert_ne!(first.last_publication_handle, 0);
        assert_eq!((second.alive_count, second.not_alive_count), (1, 0));
        assert_eq!(
            (second.alive_count_change, second.not_alive_count_change),
            (0, 0)
        );
        cleanup(p);
    }

    /// `zerodds_pub_delete_datawriter` deletes the writer, not only its
    /// handle: the reader it was matched with unmatches it and no longer
    /// counts it alive. The runtime used to keep the writer.
    #[test]
    fn deleting_a_writer_unmatches_it() {
        use crate::participant_ffi::zerodds_dp_create_publisher;
        use crate::publisher_ffi::{zerodds_pub_create_datawriter, zerodds_pub_delete_datawriter};
        let domain = u32::try_from(zerodds_test_domains::unique_domain()).expect("domain");
        let (p, sub, t) = mk(domain);
        let mut matched = ZeroDdsSubscriptionMatchedStatus::default();
        let mut live = ZeroDdsLivelinessChangedStatus::default();
        // SAFETY: p, sub and t from mk; the status slots live on the stack.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let publisher = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(publisher, t, ptr::null());
            let ok = ZeroDdsStatus::Ok as c_int;
            assert_eq!(
                zerodds_dr_get_subscription_matched_status(dr, &mut matched),
                ok
            );
            assert_eq!(matched.current_count, 1);
            assert_eq!(zerodds_pub_delete_datawriter(publisher, dw), ok);
            assert_eq!(
                zerodds_dr_get_subscription_matched_status(dr, &mut matched),
                ok
            );
            assert_eq!(zerodds_dr_get_liveliness_changed_status(dr, &mut live), ok);
        }
        assert_eq!(matched.current_count, 0);
        assert_eq!((live.alive_count, live.not_alive_count), (0, 0));
        cleanup(p);
    }

    /// A filter that reads a field, on a topic whose type the CFT does not
    /// know (no TypeObject, no schema), would pass every sample unfiltered;
    /// the reader is refused. An expression that reads no field is fine.
    #[test]
    fn cft_reader_without_type_or_schema_is_refused_for_a_field_filter() {
        use crate::participant_ffi::{
            zerodds_dp_create_contentfilteredtopic, zerodds_dp_delete_contentfilteredtopic,
        };
        let (p, sub, t) = mk(58);
        // SAFETY: p+sub+t from mk; names and expressions static.
        unsafe {
            let fields = zerodds_dp_create_contentfilteredtopic(
                p,
                c"FilteredCFT".as_ptr(),
                t,
                c"name = 'foo'".as_ptr(),
                ptr::null(),
                0,
            );
            assert!(!fields.is_null());
            let dr = zerodds_sub_create_datareader_with_cft(sub, fields, ptr::null());
            assert!(dr.is_null(), "no type, no schema: refused");
            let constant = zerodds_dp_create_contentfilteredtopic(
                p,
                c"ConstantCFT".as_ptr(),
                t,
                c"%0 = 'on'".as_ptr(),
                [c"'on'".as_ptr()].as_ptr(),
                1,
            );
            let dr = zerodds_sub_create_datareader_with_cft(sub, constant, ptr::null());
            assert!(!dr.is_null(), "an expression without fields needs no type");
            zerodds_sub_delete_datareader(sub, dr);
            zerodds_dp_delete_contentfilteredtopic(p, fields);
            zerodds_dp_delete_contentfilteredtopic(p, constant);
        }
        cleanup(p);
    }

    /// DDS 1.4 §2.2.2.2.1.8: a ContentFilteredTopic with a DataReader
    /// attached is not deleted (PRECONDITION_NOT_MET); once the reader is
    /// deleted it is.
    #[test]
    fn a_cft_with_a_reader_is_not_deleted() {
        use crate::participant_ffi::{
            zerodds_dp_create_contentfilteredtopic, zerodds_dp_delete_contentfilteredtopic,
        };
        let (p, sub, t) = mk(87);
        // SAFETY: p+sub+t from mk; the literals are static C strings.
        unsafe {
            let cft = zerodds_dp_create_contentfilteredtopic(
                p,
                c"HeldCFT".as_ptr(),
                t,
                c"%0 = 'on'".as_ptr(),
                [c"'on'".as_ptr()].as_ptr(),
                1,
            );
            assert!(!cft.is_null());
            let dr = zerodds_sub_create_datareader_with_cft(sub, cft, ptr::null());
            assert!(!dr.is_null());
            assert_eq!(
                zerodds_dp_delete_contentfilteredtopic(p, cft),
                ZeroDdsStatus::PreconditionNotMet as c_int
            );
            assert_eq!(
                zerodds_sub_delete_datareader(sub, dr),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(
                zerodds_dp_delete_contentfilteredtopic(p, cft),
                ZeroDdsStatus::Ok as c_int
            );
        }
        cleanup(p);
    }

    #[test]
    fn cft_with_invalid_expression_returns_null() {
        use crate::participant_ffi::zerodds_dp_create_contentfilteredtopic;
        let (p, sub, t) = mk(59);
        let cft_name = c"BadCFT";
        let bad_expr = c"$$invalid syntax";
        // SAFETY: p+sub+t from mk; cft_name+bad_expr static.
        unsafe {
            let cft = zerodds_dp_create_contentfilteredtopic(
                p,
                cft_name.as_ptr(),
                t,
                bad_expr.as_ptr(),
                ptr::null(),
                0,
            );
            // CFT-create succeeds (syntax not validated there).
            assert!(!cft.is_null());
            let dr = zerodds_sub_create_datareader_with_cft(sub, cft, ptr::null());
            // But create-with-cft fails on parse.
            assert!(dr.is_null(), "invalid filter syntax must reject");
        }
        cleanup(p);
    }

    #[test]
    fn read_on_empty_returns_no_data() {
        let (p, sub, t) = mk(57);
        let mut arr = ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 0,
            loan_token: ptr::null_mut(),
        };
        // SAFETY: sub+t from mk; arr lives for the test.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            let rc = zerodds_dr_read(dr, &mut arr, 10, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
        }
        cleanup(p);
    }

    #[test]
    fn dr_get_topicdescription_subscriber_roundtrip() {
        let (p, sub, t) = mk(56);
        // SAFETY: sub+t from mk.
        unsafe {
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert_eq!(zerodds_dr_get_topicdescription(dr), t);
            assert_eq!(zerodds_dr_get_subscriber(dr), sub);
        }
        cleanup(p);
    }

    // ===================================================================
    // QR-cluster regression tests (#77): exclusive-ownership arbitration,
    // CFT payload decode, keyed-lifecycle InstanceHandle on the take path.
    // ===================================================================

    use crate::entities::ZeroDdsDataReader;
    use zerodds_dcps::sample_bytes::SampleBytes;
    use zerodds_rtps::history_cache::ChangeKind;
    use zerodds_rtps::wire_types::TopicKind;

    /// Builds a standalone DataReader with a controlled sample channel so a
    /// test can inject `UserSample`s with explicit writer GUID/strength —
    /// i.e. a real same-runtime writer→reader observation of the QoS effect
    /// on the C-FFI take path. `rt`/`eid` come from the participant; the
    /// reader is NOT registered in the subscriber list, so the test drops it
    /// explicitly.
    fn mk_channel_reader(
        p: *mut ZeroDdsDomainParticipant,
        sub: *mut ZeroDdsSubscriber,
        topic: *mut ZeroDdsTopic,
        ownership: zerodds_dcps::qos::OwnershipKind,
        cft_filter: Option<crate::entities::CftFilter>,
    ) -> (*mut ZeroDdsDataReader, mpsc::Sender<UserSample>) {
        mk_channel_reader_kind(p, sub, topic, ownership, cft_filter, TopicKind::WithKey)
    }

    /// Like [`mk_channel_reader`], but the reader endpoint declares the given
    /// [`TopicKind`] in its own RTPS `EntityId` (DDSI-RTPS 2.5 §9.3.1.2
    /// Table 9.1) — the bit the take path reads the topic's keyedness from.
    fn mk_channel_reader_kind(
        p: *mut ZeroDdsDomainParticipant,
        sub: *mut ZeroDdsSubscriber,
        topic: *mut ZeroDdsTopic,
        ownership: zerodds_dcps::qos::OwnershipKind,
        cft_filter: Option<crate::entities::CftFilter>,
        topic_kind: TopicKind,
    ) -> (*mut ZeroDdsDataReader, mpsc::Sender<UserSample>) {
        // SAFETY: p from mk; dp.rt is Some for an online participant.
        let rt = unsafe { (*p).rt.as_ref().expect("participant runtime").clone() };
        let eid = match topic_kind {
            TopicKind::WithKey => {
                zerodds_rtps::wire_types::EntityId::user_reader_with_key([9, 9, 9])
            }
            TopicKind::NoKey => zerodds_rtps::wire_types::EntityId::user_reader_no_key([9, 9, 9]),
        };
        let (tx, rx) = mpsc::channel::<UserSample>();
        let dr = Box::new(ZeroDdsDataReader {
            subscriber: sub,
            topic,
            rt,
            eid,
            // KEEP_ALL: these tests feed several samples of an instance and
            // look at ownership and instances, not at HISTORY (DDS 1.4
            // §2.2.3.18; the default KEEP_LAST(1) would keep the newest).
            qos: Mutex::new(DataReaderQos {
                history: zerodds_qos::HistoryQosPolicy {
                    kind: zerodds_qos::HistoryKind::KeepAll,
                    depth: 1,
                },
                ..DataReaderQos::default()
            }),
            rx: Mutex::new(rx),
            coherent_hold: Default::default(),
            group_inbox: Default::default(),
            read_cache: Mutex::new(Vec::new()),
            held_back: Mutex::new(Vec::new()),
            cft_filter,
            cft_hold: None,
            query_type: Mutex::new(None),
            ownership,
            instances: zerodds_dcps::instance_tracker::InstanceTracker::new(),
            partition_out: Mutex::new(Default::default()),
            lifecycle: crate::entity_ffi::Lifecycle::new(true),
        });
        (Box::into_raw(dr), tx)
    }

    fn alive(payload: Vec<u8>, guid_byte: u8, strength: i32) -> UserSample {
        let mut g = [0u8; 16];
        g[0] = guid_byte;
        UserSample::Alive {
            payload: SampleBytes::from_vec(payload),
            key_hash: None,
            writer_guid: g,
            writer_strength: strength,
            representation: 1,
            big_endian: false,
            source_timestamp: None,
            reception_timestamp: None,
            source_sequence_number: -1,
            coherent_set_sn: None,
            // Test fixture: no LIFESPAN — `0` == INFINITE.
            lifespan_nanos: 0,
            related_sample_identity: None,
            group: None,
            crypto_content: false,
        }
    }

    fn empty_arr() -> ZeroDdsSampleArray {
        ZeroDdsSampleArray {
            buffers: ptr::null_mut(),
            lengths: ptr::null_mut(),
            infos: ptr::null_mut(),
            count: 0,
            loan_token: ptr::null_mut(),
        }
    }

    #[test]
    fn explicit_key_survives_non_key_changes_and_matches_lifecycle() {
        let (p, sub, t) = mk(67);
        let (dr, _tx) =
            mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Shared, None);
        // Legal zero-padded KeyHashes for string<8> "abcdX" / "abcdY".
        // Their first eight bytes match, so truncation collapses them.
        let key_a = [
            0, 0, 0, 6, b'a', b'b', b'c', b'd', b'X', 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut key_b = key_a;
        key_b[8] = b'Y';
        let keyed = |payload, key| {
            let mut sample = alive(payload, 1, 0);
            if let UserSample::Alive { key_hash, .. } = &mut sample {
                *key_hash = Some(key);
            }
            sample
        };
        let samples = vec![
            keyed(vec![1], key_a),
            keyed(vec![2], key_a),
            keyed(vec![2], key_b),
            keyed(vec![3], [0; 16]),
            UserSample::Lifecycle {
                key_hash: key_a,
                kind: ChangeKind::NotAliveDisposed,
                key_holder: None,
                source_timestamp: None,
                group: None,
                writer_guid: [0; 16],
                coherent_set_sn: None,
                source_sequence_number: -1,
            },
        ];
        // SAFETY: the channel reader is owned here and remains live until cleanup.
        let resolved = unsafe { resolve_instances_and_ownership(&*dr, samples) }
            .expect("within instance limit");
        let handles: Vec<_> = resolved
            .iter()
            .map(|(_, info)| info.instance_handle.as_raw())
            .collect();
        assert_eq!(handles.len(), 5);
        assert_eq!(handles[0], handles[1]);
        assert_ne!(handles[1], handles[2]);
        assert_ne!(
            handles[3], 0,
            "a legitimate keyed zero hash must not become HANDLE_NIL"
        );
        assert_eq!(handles[0], handles[4]);
        // SAFETY: reader and output handles remain live throughout this test.
        unsafe {
            for (key, expected) in [
                (key_a, handles[0]),
                (key_b, handles[2]),
                ([0; 16], handles[3]),
            ] {
                let mut found = 0;
                assert_eq!(
                    crate::extra_ffi::zerodds_dr_lookup_instance(dr, key.as_ptr(), 16, &mut found),
                    ZeroDdsStatus::Ok as c_int
                );
                assert_eq!(found, expected);
            }
        }
        // SAFETY: this test owns the boxed channel reader.
        unsafe { drop(Box::from_raw(dr)) };
        cleanup(p);
    }

    /// EXCLUSIVE ownership (Spec §2.2.3.23): two writers send to the SAME
    /// instance (identical payload-key). The stronger writer (strength 20)
    /// owns the instance; the weaker writer's (strength 5) sample on that
    /// instance is filtered out on the take path. Routes through the
    /// validated dcps `InstanceTracker::should_accept_sample_under_exclusive_ownership`.
    #[test]
    fn exclusive_ownership_drops_weaker_writer_on_take() {
        let (p, sub, t) = mk(60);
        let (dr, tx) =
            mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Exclusive, None);
        // Same instance payload from a strong writer first.
        let inst = alloc::vec![0x10u8, 0x20, 0x30, 0x40];
        tx.send(alive(inst.clone(), 0x01, 20)).unwrap();
        // Weaker writer, same instance → must be rejected.
        tx.send(alive(inst.clone(), 0x02, 5)).unwrap();
        // A second sample from the strong owner → accepted.
        tx.send(alive(inst.clone(), 0x01, 20)).unwrap();

        let mut arr = empty_arr();
        // SAFETY: dr+arr valid for the test.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        // Only the two strong-writer samples survive; weaker one filtered.
        assert_eq!(arr.count, 2, "weaker writer's sample must be dropped");
        // SAFETY: arr from take; dr valid.
        unsafe {
            let pubh = (*arr.infos.add(0)).publication_handle;
            assert_eq!(
                (*arr.infos.add(1)).publication_handle,
                pubh,
                "both surviving samples come from the same (strong) writer"
            );
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// SHARED ownership (default): both writers' samples are delivered (no
    /// arbitration). Confirms the filter is gated on EXCLUSIVE only.
    #[test]
    fn shared_ownership_delivers_all_writers() {
        let (p, sub, t) = mk(61);
        let (dr, tx) = mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Shared, None);
        let inst = alloc::vec![1u8, 2, 3, 4];
        tx.send(alive(inst.clone(), 0x01, 20)).unwrap();
        tx.send(alive(inst.clone(), 0x02, 5)).unwrap();
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 2, "shared ownership delivers every writer");
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// CFT (Spec §2.2.2.3.3): a `seq > 4` filter with an Int32 schema on the
    /// leading member genuinely filters the decoded payload. Samples with
    /// seq <= 4 are dropped; seq > 4 pass.
    /// The `CompleteTypeObject` of `idl`'s `name`, in the `to_bytes_le` form
    /// `idlc cpp` emits as `topic_type_support<T>::type_object()`.
    fn complete_type_object(idl: &str, name: &str) -> Vec<u8> {
        let parsed = zerodds_idl_parse::parse(idl);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "cft.idl");
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let mut ast = lowered.ast;
        assert!(ast.resolve_all_by_name().expect("resolve").is_clean());
        let ast = std::sync::Arc::new(ast);
        let id = zerodds_types::dynamic::DynamicType::find(&ast, name)
            .expect("type")
            .id();
        zerodds_types::TypeObject::Complete(
            zerodds_types::from_ast::complete_type_object(&ast, id).expect("object"),
        )
        .to_bytes_le()
        .expect("bytes")
    }

    /// `seq` (long), `value` (double), `label` (string) as XCDR2 (MAXALIGN 4)
    /// or XCDR1 (MAXALIGN 8) little-endian body of a `@final` struct.
    fn reading_body(seq: i32, value: f64, label: &str, xcdr1: bool) -> Vec<u8> {
        let mut out = seq.to_le_bytes().to_vec();
        if xcdr1 {
            out.extend_from_slice(&[0; 4]);
        }
        out.extend_from_slice(&value.to_le_bytes());
        out.extend_from_slice(&(label.len() as u32 + 1).to_le_bytes());
        out.extend_from_slice(label.as_bytes());
        out.push(0);
        out
    }

    /// A ContentFilteredTopic that knows its type from the
    /// `CompleteTypeObject` reads every field by name, whatever its position
    /// and whatever representation the sample came in.
    #[test]
    fn cft_with_a_type_object_filters_on_any_field() {
        use crate::entities::{CftExtensibility, CftFilter};
        use crate::participant_ffi::{
            zerodds_cft_set_type_object, zerodds_dp_create_contentfilteredtopic,
        };
        let idl = "@final struct SubTy { long seq; double value; string label; };";
        let object = complete_type_object(idl, "SubTy");
        let (p, sub, t) = mk(70);
        // SAFETY: p+sub+t from mk; strings static; object lives for the call.
        let dr = unsafe {
            let cft = zerodds_dp_create_contentfilteredtopic(
                p,
                c"Readings".as_ptr(),
                t,
                c"label LIKE 'ok%' AND value > %0".as_ptr(),
                [c"1.5".as_ptr()].as_ptr(),
                1,
            );
            assert!(!cft.is_null());
            assert_eq!(
                zerodds_cft_set_type_object(cft, object.as_ptr(), object.len(), 1, 0),
                ZeroDdsStatus::BadParameter as c_int,
                "the type has no key"
            );
            assert_eq!(
                zerodds_cft_set_type_object(cft, object.as_ptr(), object.len(), 0, 0),
                ZeroDdsStatus::Ok as c_int
            );
            let dr = zerodds_sub_create_datareader_with_cft(sub, cft, ptr::null());
            assert!(!dr.is_null(), "a typed CFT reader is created");
            let typed = (*cft).typed.lock().unwrap().clone();
            zerodds_sub_delete_datareader(sub, dr);
            typed
        };
        let filter = CftFilter {
            expr: zerodds_sql_filter::parse("label LIKE 'ok%' AND value > %0").unwrap(),
            params: std::sync::Arc::new(std::sync::RwLock::new(vec![
                zerodds_sql_filter::Value::from_parameter("1.5"),
            ])),
            schema: Vec::new(),
            extensibility: CftExtensibility::Final,
            typed: dr,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );
        for (seq, value, label, xcdr1) in [
            (1, 2.0, "ok-a", false),
            (2, 1.0, "ok-b", false),
            (3, 9.0, "bad", false),
            (4, 3.0, "ok-c", true),
        ] {
            let mut sample = alive(reading_body(seq, value, label, xcdr1), 0x01, 0);
            if let UserSample::Alive { representation, .. } = &mut sample {
                *representation = if xcdr1 { 0 } else { 1 };
            }
            tx.send(sample).unwrap();
        }
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 2, "seq 1 (XCDR2) and seq 4 (XCDR1) pass");
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
        }
        cleanup(p);
    }

    /// A type that refers to another: without the dependency the object is
    /// refused, with it the filter reads a nested member (`value.id`).
    #[test]
    fn cft_type_with_a_dependency_filters_a_nested_member() {
        use crate::entities::{CftExtensibility, CftFilter};
        use crate::participant_ffi::{
            zerodds_cft_set_type_object, zerodds_cft_set_type_objects,
            zerodds_dp_create_contentfilteredtopic,
        };
        let idl = "@final struct Leaf { long id; }; @final struct SubTy { Leaf value; long seq; };";
        let root = complete_type_object(idl, "SubTy");
        let leaf = complete_type_object(idl, "Leaf");
        let (p, sub, t) = mk(72);
        // SAFETY: p+t from mk; strings static; objects live for the calls.
        let typed = unsafe {
            let cft = zerodds_dp_create_contentfilteredtopic(
                p,
                c"LeafAbove".as_ptr(),
                t,
                c"value.id > 3".as_ptr(),
                ptr::null(),
                0,
            );
            assert!(!cft.is_null());
            assert_eq!(
                zerodds_cft_set_type_object(cft, root.as_ptr(), root.len(), 0, 0),
                ZeroDdsStatus::BadParameter as c_int,
                "Leaf is missing"
            );
            let deps = [leaf.as_ptr()];
            let lens = [leaf.len()];
            assert_eq!(
                zerodds_cft_set_type_objects(
                    cft,
                    root.as_ptr(),
                    root.len(),
                    deps.as_ptr(),
                    lens.as_ptr(),
                    1,
                    0,
                    0
                ),
                ZeroDdsStatus::Ok as c_int
            );
            (*cft).typed.lock().unwrap().clone()
        };
        let filter = CftFilter {
            expr: zerodds_sql_filter::parse("value.id > 3").unwrap(),
            params: std::sync::Arc::default(),
            schema: Vec::new(),
            extensibility: CftExtensibility::Final,
            typed,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );
        for (id, seq) in [(1i32, 10i32), (5, 11), (9, 12)] {
            let mut body = id.to_le_bytes().to_vec();
            body.extend_from_slice(&seq.to_le_bytes());
            tx.send(alive(body, 0x01, 0)).unwrap();
        }
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        assert_eq!(
            unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) },
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(arr.count, 2, "value.id 5 and 9");
        // SAFETY: cleanup.
        unsafe { zerodds_dr_return_loan(dr, &mut arr) };
        cleanup(p);
    }

    /// New parameters reach a reader that shares the topic's values
    /// (DDS 1.4 §2.2.2.3.3), and the getters return them.
    #[test]
    fn cft_parameters_set_later_reach_the_reader() {
        use crate::entities::{CftExtensibility, CftField, CftFieldKind, CftFilter};
        use crate::participant_ffi::{
            zerodds_cft_get_filter_parameter, zerodds_cft_get_filter_parameter_count,
            zerodds_cft_set_filter_parameters, zerodds_dp_create_contentfilteredtopic,
        };
        use crate::topic_ffi::zerodds_string_free;
        let (p, sub, t) = mk(71);
        // SAFETY: p+t from mk; strings static.
        let cft = unsafe {
            zerodds_dp_create_contentfilteredtopic(
                p,
                c"SeqAbove".as_ptr(),
                t,
                c"seq > %0".as_ptr(),
                [c"4".as_ptr()].as_ptr(),
                1,
            )
        };
        assert!(!cft.is_null());
        let filter = CftFilter {
            expr: zerodds_sql_filter::parse("seq > %0").unwrap(),
            // SAFETY: cft valid.
            params: std::sync::Arc::clone(unsafe { &(*cft).values }),
            schema: alloc::vec![CftField {
                name: "seq".into(),
                kind: CftFieldKind::Int32,
            }],
            extensibility: CftExtensibility::Final,
            typed: None,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );
        let take = |tx: &mpsc::Sender<UserSample>| {
            for v in [3i32, 7, 5] {
                tx.send(alive(v.to_le_bytes().to_vec(), 0x01, 0)).unwrap();
            }
            let mut arr = empty_arr();
            // SAFETY: dr+arr valid.
            unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
            let n = arr.count;
            // SAFETY: cleanup of the loan.
            unsafe { zerodds_dr_return_loan(dr, &mut arr) };
            n
        };
        assert_eq!(take(&tx), 2, "seq > 4: 7 and 5");
        // SAFETY: cft valid; parameters static.
        unsafe {
            assert_eq!(
                zerodds_cft_set_filter_parameters(cft, ptr::null(), 0),
                ZeroDdsStatus::BadParameter as c_int,
                "%0 without a parameter"
            );
            assert_eq!(
                zerodds_cft_set_filter_parameters(cft, [c"6".as_ptr()].as_ptr(), 1),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(zerodds_cft_get_filter_parameter_count(cft), 1);
            let got = zerodds_cft_get_filter_parameter(cft, 0);
            assert_eq!(std::ffi::CStr::from_ptr(got).to_str().unwrap(), "6");
            zerodds_string_free(got);
            assert!(zerodds_cft_get_filter_parameter(cft, 1).is_null());
        }
        assert_eq!(take(&tx), 1, "seq > 6: only 7");
        cleanup(p);
    }

    #[test]
    fn cft_seq_filter_decodes_and_filters_payload() {
        use crate::entities::{CftField, CftFieldKind, CftFilter};
        let (p, sub, t) = mk(62);
        let expr = zerodds_sql_filter::parse("seq > 4").expect("parse seq>4");
        let filter = CftFilter {
            expr,
            params: std::sync::Arc::default(),
            schema: alloc::vec![CftField {
                name: "seq".into(),
                kind: CftFieldKind::Int32,
            }],
            extensibility: crate::entities::CftExtensibility::Final,
            typed: None,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );
        // XCDR2 little-endian i32 payloads: 3 (drop), 7 (keep), 5 (keep).
        for v in [3i32, 7, 5] {
            tx.send(alive(v.to_le_bytes().to_vec(), 0x01, 0)).unwrap();
        }
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 2, "only seq=7 and seq=5 pass seq>4");
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// CFT string field: `name = 'foo'` over a length-prefixed CDR string
    /// payload genuinely filters.
    #[test]
    fn cft_string_filter_decodes_payload() {
        use crate::entities::{CftField, CftFieldKind, CftFilter};
        let (p, sub, t) = mk(63);
        let expr = zerodds_sql_filter::parse("name = 'foo'").expect("parse name=foo");
        let filter = CftFilter {
            expr,
            params: std::sync::Arc::default(),
            schema: alloc::vec![CftField {
                name: "name".into(),
                kind: CftFieldKind::StringField,
            }],
            extensibility: crate::entities::CftExtensibility::Final,
            typed: None,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );
        // CDR string: u32 len (incl NUL) + bytes + NUL, XCDR LE.
        let cdr_string = |s: &str| -> Vec<u8> {
            let mut v = Vec::new();
            let len = (s.len() + 1) as u32;
            v.extend_from_slice(&len.to_le_bytes());
            v.extend_from_slice(s.as_bytes());
            v.push(0);
            v
        };
        tx.send(alive(cdr_string("foo"), 0x01, 0)).unwrap();
        tx.send(alive(cdr_string("bar"), 0x01, 0)).unwrap();
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 1, "only name='foo' passes");
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// QB-rest regression (#79, issue 3): CFT untyped positional schema for an
    /// `@appendable` type. In XCDR2 an @appendable/@mutable aggregate is
    /// prefixed with a 4-byte DHEADER (XTypes 1.3 §7.4.3.4.2) before the first
    /// member, so the positional decoder must skip those 4 bytes — otherwise it
    /// reads the DHEADER as the first column and every comparison resolves at
    /// the wrong offset. With `CftExtensibility::Appendable` set, both a numeric
    /// (`seq` int32) and a string (`name`) column resolve, so the filter
    /// `seq > 4 AND name = 'keep'` genuinely filters.
    #[test]
    fn cft_appendable_dheader_offset_resolves_columns() {
        use crate::entities::{CftExtensibility, CftField, CftFieldKind, CftFilter};
        let (p, sub, t) = mk(65);
        let expr = zerodds_sql_filter::parse("seq > 4 AND name = 'keep'")
            .expect("parse appendable filter");
        let filter = CftFilter {
            expr,
            params: std::sync::Arc::default(),
            schema: alloc::vec![
                CftField {
                    name: "seq".into(),
                    kind: CftFieldKind::Int32,
                },
                CftField {
                    name: "name".into(),
                    kind: CftFieldKind::StringField,
                },
            ],
            // @appendable → leading 4-byte DHEADER is skipped.
            extensibility: CftExtensibility::Appendable,
            typed: None,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );

        // Build an @appendable XCDR2 body: 4-byte DHEADER (object length) then
        // the members. After the i32 `seq`, the `name` string is 4-byte aligned
        // (already aligned here: 4 DHEADER + 4 seq = offset 8). CDR string =
        // u32 length (incl NUL) + bytes + NUL.
        let appendable = |seq: i32, name: &str| -> Vec<u8> {
            let mut body = Vec::new();
            body.extend_from_slice(&seq.to_le_bytes());
            let len = (name.len() + 1) as u32;
            body.extend_from_slice(&len.to_le_bytes());
            body.extend_from_slice(name.as_bytes());
            body.push(0);
            // DHEADER = byte length of the member block that follows.
            let mut v = Vec::new();
            v.extend_from_slice(&(body.len() as u32).to_le_bytes());
            v.extend_from_slice(&body);
            v
        };

        // seq=3,"keep" → drop (seq<=4); seq=7,"drop" → drop (name); seq=9,"keep" → keep.
        tx.send(alive(appendable(3, "keep"), 0x01, 1)).unwrap();
        tx.send(alive(appendable(7, "drop"), 0x01, 1)).unwrap();
        tx.send(alive(appendable(9, "keep"), 0x01, 1)).unwrap();

        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(
            arr.count, 1,
            "only seq=9,name='keep' passes once the DHEADER offset is honored"
        );
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// Control for the DHEADER fix: the SAME @appendable payload with the
    /// extensibility left at `Final` (offset 0) reads the DHEADER as `seq` and
    /// thus does NOT resolve the columns correctly — proving the offset is
    /// load-bearing. The DHEADER value (a small object length) is < 5, so a
    /// `seq > 4` filter on the misread offset rejects the sample.
    #[test]
    fn cft_appendable_payload_misreads_without_dheader_skip() {
        use crate::entities::{CftExtensibility, CftField, CftFieldKind, CftFilter};
        let (p, sub, t) = mk(66);
        let expr = zerodds_sql_filter::parse("seq > 4").expect("parse seq>4");
        let filter = CftFilter {
            expr,
            params: std::sync::Arc::default(),
            schema: alloc::vec![CftField {
                name: "seq".into(),
                kind: CftFieldKind::Int32,
            }],
            // WRONG on purpose: payload is @appendable but declared @final.
            extensibility: CftExtensibility::Final,
            typed: None,
        };
        let (dr, tx) = mk_channel_reader(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            Some(filter),
        );
        // @appendable body: DHEADER(=4) + seq(=9). Read at offset 0 the decoder
        // sees the DHEADER (4) as `seq`, so `seq > 4` is FALSE and the sample is
        // dropped — even though the real seq is 9.
        let mut payload = Vec::new();
        payload.extend_from_slice(&4u32.to_le_bytes()); // DHEADER
        payload.extend_from_slice(&9i32.to_le_bytes()); // real seq
        tx.send(alive(payload, 0x01, 1)).unwrap();
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        // Misread offset → seq looks like 4 → seq>4 false → NoData.
        assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
        assert_eq!(arr.count, 0);
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// Keyed lifecycle (Spec §2.2.2.5.4): the take path surfaces a real,
    /// non-zero, per-instance InstanceHandle. Two distinct payloads yield two
    /// distinct handles; a DISPOSED lifecycle marker for an instance carries
    /// the SAME handle as that instance's alive sample AND reports
    /// instance_state = NOT_ALIVE_DISPOSED (2).
    #[test]
    fn keyed_lifecycle_surfaces_real_instance_handle() {
        let (p, sub, t) = mk(64);
        let (dr, tx) = mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Shared, None);
        let inst_a = alloc::vec![0xAAu8, 0xAA, 0xAA, 0xAA];
        let inst_b = alloc::vec![0xBBu8, 0xBB, 0xBB, 0xBB];
        tx.send(alive(inst_a.clone(), 0x01, 0)).unwrap();
        tx.send(alive(inst_b.clone(), 0x01, 0)).unwrap();

        let mut arr = empty_arr();
        // SAFETY: dr+arr valid.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 10, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 2);
        // SAFETY: take returned count==2, so infos[0..2] are initialized.
        let (handle_a, handle_b) = unsafe {
            let h0 = (*arr.infos.add(0)).instance_handle;
            let h1 = (*arr.infos.add(1)).instance_handle;
            (h0, h1)
        };
        assert_ne!(handle_a, 0, "alive instance handle must be non-zero");
        assert_ne!(handle_b, 0, "alive instance handle must be non-zero");
        assert_ne!(
            handle_a, handle_b,
            "distinct instances must have distinct handles"
        );
        // SAFETY: return first loan before the next take.
        unsafe { zerodds_dr_return_loan(dr, &mut arr) };

        // Now dispose instance A via a Lifecycle marker carrying its KeyHash.
        // The C-FFI derives the alive instance handle from the payload; the
        // dispose path must produce the SAME handle so the transition is
        // correlatable. We reconstruct A's payload-key hash the same way the
        // take path does.
        let kh_a = payload_key_hash(&inst_a);
        tx.send(UserSample::Lifecycle {
            key_hash: kh_a,
            kind: ChangeKind::NotAliveDisposed,
            key_holder: None,
            source_timestamp: None,
            group: None,
            writer_guid: [0; 16],
            coherent_set_sn: None,
            source_sequence_number: -1,
        })
        .unwrap();
        let mut arr2 = empty_arr();
        // SAFETY: dr+arr2 valid.
        let rc2 = unsafe { zerodds_dr_take(dr, &mut arr2, 10, 0, 0, 0) };
        assert_eq!(rc2, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr2.count, 1);
        // SAFETY: arr2 from take.
        unsafe {
            let info = *arr2.infos.add(0);
            assert_eq!(
                info.instance_state, 2,
                "DISPOSED → NOT_ALIVE_DISPOSED state"
            );
            assert!(!info.valid_data, "lifecycle marker carries no data");
            assert_eq!(
                info.instance_handle, handle_a,
                "dispose marker handle must match the alive instance handle"
            );
            zerodds_dr_return_loan(dr, &mut arr2);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    // ===================================================================
    // Instance-tracker growth (unbounded memory on the C take path).
    //
    // `zerodds_dr_take` used to call `InstanceTracker::observe_sample` for
    // every Alive sample, with a KeyHash derived from the whole payload and
    // the payload itself stored as the entry's "key holder". Nothing ever
    // removes such an entry: `autopurge` only drops instances that are
    // NOT_ALIVE_DISPOSED / NOT_ALIVE_NO_WRITERS, which a topic with a live
    // writer never becomes. The tracker is read nowhere else in this crate
    // than by the EXCLUSIVE-ownership arbitration, so on a SHARED reader
    // (the default OWNERSHIP, §2.2.3.23) every entry was write-only.
    // ===================================================================

    /// Number of Alive samples the growth tests push through one `take`.
    /// Every payload is distinct, so the old payload-derived instance
    /// identity produced exactly this many tracker entries.
    const GROWTH_SAMPLES: usize = 512;

    fn distinct_payloads(n: usize) -> alloc::vec::Vec<Vec<u8>> {
        (0..n).map(|i| (i as u64).to_le_bytes().to_vec()).collect()
    }

    /// Keyless topic (reader endpoint entityKind `NO_KEY`, DDSI-RTPS 2.5
    /// §9.3.1.2 Table 9.1): `n` distinct payloads are `n` samples of the
    /// **one** instance the topic has (DDS 1.4 §2.2.1.2.2), so the take path
    /// must leave no per-sample state behind and must report one and the
    /// same `InstanceHandle` for all of them.
    ///
    /// Before the fix: `instances.len() == 512` after one `take` of 512
    /// samples, each entry holding a copy of its payload, and 512 distinct
    /// instance handles. After: one entry, one handle that is not
    /// HANDLE_NIL, the first sample NEW and the others NOT_NEW.
    #[test]
    fn keyless_take_does_not_grow_the_instance_tracker() {
        let (p, sub, t) = mk(12);
        let (dr, tx) = mk_channel_reader_kind(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            None,
            TopicKind::NoKey,
        );
        for payload in distinct_payloads(GROWTH_SAMPLES) {
            tx.send(alive(payload, 0x01, 0)).expect("send");
        }
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid for the test.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, GROWTH_SAMPLES, "every sample is delivered");
        // SAFETY: take returned count==GROWTH_SAMPLES, infos are initialized.
        unsafe {
            let first = (*arr.infos.add(0)).instance_handle;
            assert_ne!(first, 0, "the one instance has a handle (§2.2.1.2.2)");
            assert_eq!(
                (*arr.infos.add(0)).view_state,
                1,
                "the first sample of the instance is NEW"
            );
            for i in 1..arr.count {
                assert_eq!(
                    (*arr.infos.add(i)).instance_handle,
                    first,
                    "a keyless topic is one instance — sample {i} must carry its handle"
                );
            }
        }
        // SAFETY: dr valid.
        let tracked = unsafe { (*dr).instances.len() };
        assert_eq!(
            tracked, 1,
            "the topic's one instance is one tracker entry, not one per sample"
        );
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// DDS 1.4 §2.2.1.2.2: the one instance of a keyless topic can be
    /// disposed; the marker is a sample with `valid_data` false whose
    /// instance is the one of the data samples and whose state is
    /// NOT_ALIVE_DISPOSED (§2.2.2.5.1.3, 2).
    #[test]
    fn a_keyless_instance_can_be_disposed() {
        let (p, sub, t) = mk(88);
        let (dr, tx) = mk_channel_reader_kind(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            None,
            TopicKind::NoKey,
        );
        tx.send(alive(vec![1, 2, 3, 4], 0x01, 0)).expect("send");
        tx.send(UserSample::Lifecycle {
            key_hash: [0xAB; 16],
            kind: zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed,
            key_holder: None,
            source_timestamp: None,
            group: None,
            writer_guid: [0; 16],
            coherent_set_sn: None,
            source_sequence_number: -1,
        })
        .expect("send");
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid for the test.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 2, "the sample and the dispose marker");
        // SAFETY: take returned count==2, infos are initialized.
        unsafe {
            let data = &*arr.infos.add(0);
            let marker = &*arr.infos.add(1);
            assert_ne!(data.instance_handle, 0);
            assert_eq!(marker.instance_handle, data.instance_handle);
            assert!(!marker.valid_data);
            assert_eq!(marker.instance_state, 2, "NOT_ALIVE_DISPOSED");
        }
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// Full-key identities remain bounded even under SHARED ownership.
    #[test]
    fn keyed_take_under_shared_ownership_obeys_instance_limit() {
        let (p, sub, t) = mk(14);
        let (dr, tx) = mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Shared, None);
        // SAFETY: owned reader, before the first read/take.
        unsafe {
            (*dr).qos.lock().unwrap().resource_limits.max_instances = 8;
        }
        for payload in distinct_payloads(GROWTH_SAMPLES) {
            tx.send(alive(payload, 0x01, 0)).expect("send");
        }
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid for the test.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0) };
        // The 8 admitted instances are delivered; each refused sample is
        // counted on the reader's status (DDS 1.4 §2.2.3.19), which needs a
        // runtime reader slot this channel-only reader does not have —
        // `tests/instance_tracker_bounded.rs` checks the count.
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(
            arr.count, 8,
            "the admissible samples of the batch are delivered"
        );
        // SAFETY: dr valid.
        let tracked = unsafe { (*dr).instances.len() };
        assert_eq!(
            tracked, 8,
            "full-key identities must not exceed the configured instance limit"
        );
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// EXCLUSIVE ownership is the one consumer of the tracker on this path,
    /// so an entry may exist — but on a keyless topic there is exactly one
    /// instance to arbitrate (all-zero KeyHash: XTypes 1.3 §7.6.8 pads an
    /// empty key holder to 16 zero bytes), no matter how many samples pass.
    /// Before the fix: 512 entries and no arbitration at all, because every
    /// payload looked like a fresh instance whose first writer wins.
    #[test]
    fn keyless_exclusive_arbitration_uses_one_instance() {
        let (p, sub, t) = mk(15);
        let (dr, tx) = mk_channel_reader_kind(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Exclusive,
            None,
            TopicKind::NoKey,
        );
        // Strong writer (strength 20) and weak writer (strength 5) alternate,
        // every sample with a payload of its own.
        for (i, payload) in distinct_payloads(GROWTH_SAMPLES).into_iter().enumerate() {
            let (guid, strength) = if i % 2 == 0 { (0x01, 20) } else { (0x02, 5) };
            tx.send(alive(payload, guid, strength)).expect("send");
        }
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid for the test.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(
            arr.count,
            GROWTH_SAMPLES / 2,
            "the weaker writer owns nothing on the topic's single instance"
        );
        // SAFETY: dr valid.
        let tracked = unsafe { (*dr).instances.len() };
        assert_eq!(tracked, 1, "one keyless topic is one arbitrated instance");
        // SAFETY: cleanup.
        unsafe {
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// The take path names an instance the same way the rest of this C API
    /// does: `zerodds_dr_lookup_instance` maps a 16-byte KeyHash onto the
    /// `InstanceHandle` formed from its low 8 bytes (little-endian). Before
    /// the fix the take path handed out `InstanceTracker`-allocated counters
    /// instead, so a handle from `lookup_instance` never matched a handle in
    /// `SampleInfo` and `zerodds_dr_take_instance` filtered everything away.
    #[test]
    fn take_handle_matches_lookup_instance_handle() {
        let (p, sub, t) = mk(16);
        let (dr, tx) = mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Shared, None);
        let inst = alloc::vec![0x11u8, 0x22, 0x33, 0x44];
        tx.send(alive(inst.clone(), 0x01, 0)).expect("send");
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid for the test.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(arr.count, 1);
        let kh = payload_key_hash(&inst);
        let mut looked_up: u64 = 0;
        // SAFETY: kh is 16 bytes; looked_up is a live local.
        let lrc = unsafe {
            crate::extra_ffi::zerodds_dr_lookup_instance(dr, kh.as_ptr(), 16, &mut looked_up)
        };
        assert_eq!(lrc, ZeroDdsStatus::Ok as c_int);
        // SAFETY: arr from take with count==1.
        unsafe {
            assert_eq!(
                (*arr.infos.add(0)).instance_handle,
                looked_up,
                "take and lookup_instance must name the same instance"
            );
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// DDS 1.4 §2.2.2.5.3.15: `take_instance` takes the samples of one
    /// instance; the others stay in the reader. It took every sample and
    /// filtered the result, so the other instance's sample was gone.
    /// §2.2.2.5.3.14: `read_instance` marks only what it returns READ.
    #[test]
    fn an_instance_read_or_take_leaves_the_other_instances() {
        let (p, sub, t) = mk(69);
        let (dr, tx) = mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Shared, None);
        let a = alloc::vec![0x11u8, 0x22, 0x33, 0x44];
        let b = alloc::vec![0x55u8, 0x66, 0x77, 0x88];
        tx.send(alive(a.clone(), 0x01, 0)).expect("send a");
        tx.send(alive(b.clone(), 0x01, 0)).expect("send b");
        let handle = |payload: &[u8]| {
            let kh = payload_key_hash(payload);
            let mut h: u64 = 0;
            // SAFETY: kh is 16 bytes; h is a live local.
            let rc = unsafe {
                crate::extra_ffi::zerodds_dr_lookup_instance(dr, kh.as_ptr(), 16, &mut h)
            };
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            h
        };
        // The reader knows both instances once it has taken them in.
        let mut arr = empty_arr();
        // SAFETY: dr+arr valid; NOT_NEW selects nothing yet, the read takes the
        // samples in.
        let rc = unsafe { zerodds_dr_read(dr, &mut arr, 0, 0, 2, 0) };
        assert_eq!(rc, ZeroDdsStatus::NoData as c_int);
        let (ha, hb) = (handle(&a), handle(&b));

        // SAFETY: dr+arr valid.
        unsafe {
            let rc = crate::extra_ffi::zerodds_dr_read_instance(dr, ha, &mut arr, 0, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(arr.count, 1);
            zerodds_dr_return_loan(dr, &mut arr);
            // NOT_READ: b only — the read of a did not mark it.
            let rc = zerodds_dr_read(dr, &mut arr, 0, 2, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(arr.count, 1);
            assert_eq!((*arr.infos.add(0)).instance_handle, hb);
            zerodds_dr_return_loan(dr, &mut arr);

            let rc = crate::extra_ffi::zerodds_dr_take_instance(dr, ha, &mut arr, 0, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(arr.count, 1);
            assert_eq!((*arr.infos.add(0)).instance_handle, ha);
            zerodds_dr_return_loan(dr, &mut arr);
            let rc = zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(arr.count, 1, "the take of a took b");
            assert_eq!((*arr.infos.add(0)).instance_handle, hb);
            zerodds_dr_return_loan(dr, &mut arr);
            let _ = Box::from_raw(dr);
        }
        cleanup(p);
    }

    /// DDS 1.4 §2.2.2.5.3.17: `take_next_instance` takes the next instance
    /// only. It took every sample and handed out the first instance's.
    #[test]
    fn take_next_instance_takes_one_instance() {
        let (p, sub, t) = mk(85);
        let (dr, tx) = mk_channel_reader(p, sub, t, zerodds_dcps::qos::OwnershipKind::Shared, None);
        for payload in [[1u8, 0, 0, 0], [2, 0, 0, 0], [3, 0, 0, 0]] {
            tx.send(alive(payload.to_vec(), 0x01, 0)).expect("send");
        }
        let mut arr = empty_arr();
        let mut seen = Vec::new();
        let mut previous = 0u64;
        // SAFETY: dr+arr valid.
        unsafe {
            loop {
                let rc = crate::extra_ffi::zerodds_dr_take_next_instance(
                    dr, previous, &mut arr, 0, 0, 0, 0,
                );
                if rc == ZeroDdsStatus::NoData as c_int {
                    break;
                }
                assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
                assert_eq!(arr.count, 1, "one instance per call");
                previous = (*arr.infos.add(0)).instance_handle;
                seen.push(previous);
                zerodds_dr_return_loan(dr, &mut arr);
            }
            let _ = Box::from_raw(dr);
        }
        assert_eq!(seen.len(), 3, "an instance was lost: {seen:?}");
        cleanup(p);
    }

    /// The topic surface can declare a keyless reader, and the declaration
    /// lands where the take path reads it: the endpoint's own `entityKind`
    /// (DDSI-RTPS 2.5 §9.3.1.2 Table 9.1). Without this entry point a keyless
    /// topic could not be expressed on the DataReader surface at all — every
    /// reader was announced `WITH_KEY`.
    #[test]
    fn create_datareader_kind_sets_the_endpoint_topic_kind() {
        let (p, sub, t) = mk(68);
        // SAFETY: sub+t from mk, valid until cleanup.
        unsafe {
            let keyed = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert!(!keyed.is_null());
            assert_eq!(
                (*keyed).eid.entity_kind.topic_kind(),
                Some(TopicKind::WithKey),
                "the plain create stays WITH_KEY"
            );
            let keyless = zerodds_sub_create_datareader_kind(sub, t, ptr::null(), 0);
            assert!(!keyless.is_null());
            assert_eq!(
                (*keyless).eid.entity_kind.topic_kind(),
                Some(TopicKind::NoKey),
                "is_keyed=0 announces a NO_KEY reader endpoint"
            );
            assert_eq!(reader_topic_kind(&*keyless), TopicKind::NoKey);
        }
        cleanup(p);
    }

    /// A keyless reader over an injected channel with `history`,
    /// `max_samples_per_instance` and `reliable`, fed `[1]`, `[2]`, `[3]`.
    fn keyless_reader_fed_three(
        domain: u32,
        history: zerodds_qos::HistoryQosPolicy,
        max_samples_per_instance: i32,
        reliable: bool,
    ) -> (*mut ZeroDdsDomainParticipant, *mut ZeroDdsDataReader) {
        let (p, sub, t) = mk(domain);
        let (dr, tx) = mk_channel_reader_kind(
            p,
            sub,
            t,
            zerodds_dcps::qos::OwnershipKind::Shared,
            None,
            TopicKind::NoKey,
        );
        // SAFETY: dr from mk_channel_reader_kind, live until cleanup.
        unsafe {
            let mut qos = (*dr).qos.lock().unwrap();
            qos.history = history;
            qos.resource_limits.max_samples_per_instance = max_samples_per_instance;
            qos.reliability.kind = if reliable {
                ReliabilityKind::Reliable
            } else {
                ReliabilityKind::BestEffort
            };
        }
        for payload in [1u8, 2, 3] {
            tx.send(alive(alloc::vec![payload], 1, 0)).unwrap();
        }
        (p, dr)
    }

    /// The payloads of one take of `dr`.
    fn take_payloads(dr: *mut ZeroDdsDataReader) -> Vec<Vec<u8>> {
        let mut arr = empty_arr();
        // SAFETY: dr live; arr a valid out-parameter whose arrays hold
        // `count` entries until the loan is returned.
        unsafe {
            let _ = zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0);
            let out = (0..arr.count)
                .map(|i| slice::from_raw_parts(*arr.buffers.add(i), *arr.lengths.add(i)).to_vec())
                .collect();
            if arr.count > 0 {
                zerodds_dr_return_loan(dr, &mut arr);
            }
            out
        }
    }

    /// DDS 1.4 §2.2.3.18 KEEP_LAST(depth): "the Service will only attempt
    /// to keep the latest values of the instance". The C reader kept all.
    #[test]
    fn keep_last_keeps_the_newest_of_the_instance() {
        let (p, dr) = keyless_reader_fed_three(
            89,
            zerodds_qos::HistoryQosPolicy {
                kind: zerodds_qos::HistoryKind::KeepLast,
                depth: 2,
            },
            -1,
            false,
        );
        assert_eq!(
            take_payloads(dr),
            alloc::vec![alloc::vec![2u8], alloc::vec![3]]
        );
        // SAFETY: dr from mk_channel_reader_kind, not in the subscriber's list.
        unsafe { drop(Box::from_raw(dr)) };
        cleanup(p);
    }

    /// §2.2.3.19 under BEST_EFFORT: over `max_samples_per_instance` the
    /// oldest goes, as the typed reader evicts it.
    #[test]
    fn a_best_effort_reader_over_its_instance_limit_evicts_the_oldest() {
        let (p, dr) = keyless_reader_fed_three(
            87,
            zerodds_qos::HistoryQosPolicy {
                kind: zerodds_qos::HistoryKind::KeepAll,
                depth: 1,
            },
            2,
            false,
        );
        assert_eq!(
            take_payloads(dr),
            alloc::vec![alloc::vec![2u8], alloc::vec![3]]
        );
        // SAFETY: as above.
        unsafe { drop(Box::from_raw(dr)) };
        cleanup(p);
    }

    /// §2.2.3.19 and §2.2.4.1 under RELIABLE: the sample over
    /// `max_samples_per_instance` is refused, "a rejected sample may be
    /// accepted in the future", and comes with the next take.
    #[test]
    fn a_reliable_reader_over_its_instance_limit_holds_the_sample_back() {
        let (p, dr) = keyless_reader_fed_three(
            88,
            zerodds_qos::HistoryQosPolicy {
                kind: zerodds_qos::HistoryKind::KeepAll,
                depth: 1,
            },
            2,
            true,
        );
        assert_eq!(
            take_payloads(dr),
            alloc::vec![alloc::vec![1u8], alloc::vec![2]]
        );
        assert_eq!(take_payloads(dr), alloc::vec![alloc::vec![3u8]]);
        // SAFETY: as above.
        unsafe { drop(Box::from_raw(dr)) };
        cleanup(p);
    }
}
