// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Publisher + DataWriter C-FFI (Spec §2.2.2.4 + DDS-PSM-Cxx §7.5.5/§7.5.7).

use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::c_int;
use core::ptr;
use core::slice;
use std::sync::Mutex;
use std::time::Duration;

use zerodds_dcps::qos::{DataWriterQos, DurabilityKind, ReliabilityKind};
use zerodds_dcps::runtime::{DcpsRuntime, UserWriterConfig};
use zerodds_rtps::wire_types::EntityId;
use zerodds_rtps::writer_group::WriterGroup;

use crate::ZeroDdsStatus;
use crate::entities::{
    ZeroDdsDataWriter, ZeroDdsDomainParticipant, ZeroDdsPublisher, ZeroDdsTopic,
};
use crate::qos_ffi::{ZeroDdsDataWriterQos, dw_qos_from_c};

// ---------------------------------------------------------------------------
// Publisher: lookup / participant / suspend / resume
// ---------------------------------------------------------------------------

/// Returns the owning participant.
///
/// # Safety
/// `pub_` valid.
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

/// Suspend publications (DDS 1.4 §2.2.2.4.1.8), a hint. Each call wants
/// its own `resume_publications`.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_suspend_publications(pub_: *mut ZeroDdsPublisher) -> c_int {
    // SAFETY: a non-null `pub_` is a live publisher handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(pub_) } {
        return rc;
    }
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above.
    let Ok(mut g) = (unsafe { &*pub_ }).suspended.lock() else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    *g = g.saturating_add(1);
    ZeroDdsStatus::Ok as c_int
}

/// Resume publications (DDS 1.4 §2.2.2.4.1.9): "The call to
/// resume_publications must match a previous call to suspend_publications.
/// Otherwise the operation will return the error PRECONDITION_NOT_MET." It
/// returned OK.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_resume_publications(pub_: *mut ZeroDdsPublisher) -> c_int {
    // SAFETY: a non-null `pub_` is a live publisher handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(pub_) } {
        return rc;
    }
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above.
    let Ok(mut g) = (unsafe { &*pub_ }).suspended.lock() else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    match g.checked_sub(1) {
        Some(n) => {
            *g = n;
            ZeroDdsStatus::Ok as c_int
        }
        None => ZeroDdsStatus::PreconditionNotMet as c_int,
    }
}

/// `begin_coherent_changes` (DDS 1.4 §2.2.2.4.1.10): opens a coherent set
/// on every DataWriter of the publisher; their changes carry
/// `PID_COHERENT_SET` until the matching `end_coherent_changes`
/// (DDSI-RTPS 2.5 §8.7.6). The calls nest. A writer created while the set is
/// open joins it. Under PRESENTATION `access_scope` GROUP the set spans the
/// writers and its changes carry `PID_GROUP_COHERENT_SET` (§8.7.5).
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_begin_coherent_changes(pub_: *mut ZeroDdsPublisher) -> c_int {
    // SAFETY: a non-null `pub_` is a live publisher handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(pub_) } {
        return rc;
    }
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above; the writers come
    // from pub_create_datawriter (Box::into_raw).
    unsafe {
        let pp = &*pub_;
        let Ok(mut depth) = pp.coherent_depth.lock() else {
            return ZeroDdsStatus::Error as c_int;
        };
        *depth += 1;
        if *depth > 1 {
            return ZeroDdsStatus::Ok as c_int;
        }
        if let Some(group) = writer_group(pp) {
            group.begin_coherent_set();
        }
        for dw in writers_of(pp) {
            if (*dw).rt.begin_user_writer_coherent_set((*dw).eid).is_err() {
                *depth -= 1;
                return ZeroDdsStatus::Error as c_int;
            }
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// `end_coherent_changes` (DDS 1.4 §2.2.2.4.1.9): the last of the nested
/// calls closes the set on every writer and sends the end-of-coherent-set
/// marker (DDSI-RTPS 2.5 §8.7.6); a writer that wrote nothing in the set
/// sends none. Without an open set `ZERODDS_PRECONDITION_NOT_MET`.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_end_coherent_changes(pub_: *mut ZeroDdsPublisher) -> c_int {
    // SAFETY: a non-null `pub_` is a live publisher handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(pub_) } {
        return rc;
    }
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above; the writers come
    // from pub_create_datawriter (Box::into_raw).
    unsafe {
        let pp = &*pub_;
        let Ok(mut depth) = pp.coherent_depth.lock() else {
            return ZeroDdsStatus::Error as c_int;
        };
        if *depth == 0 {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        *depth -= 1;
        if *depth > 0 {
            return ZeroDdsStatus::Ok as c_int;
        }
        // GROUP scope: the set spans the writers, and each of them sends an
        // end-of-coherent-set DATA at the same group sequence number
        // (DDSI-RTPS 2.5 §8.7.6). A set that holds no change ends without.
        let group_end = existing_writer_group(pp).and_then(|group| group.end_coherent_set());
        let mut status = ZeroDdsStatus::Ok;
        for dw in writers_of(pp) {
            let ended = match group_end {
                Some(end) => (*dw).rt.end_user_writer_group_coherent_set((*dw).eid, end),
                None => (*dw).rt.end_user_writer_coherent_set((*dw).eid),
            };
            if ended.is_err() {
                status = ZeroDdsStatus::Error;
            }
        }
        status as c_int
    }
}

/// The publisher's PRESENTATION has `access_scope` GROUP.
fn group_scope(pp: &ZeroDdsPublisher) -> bool {
    pp.qos.lock().is_ok_and(|q| {
        matches!(
            q.presentation.access_scope,
            zerodds_qos::PresentationAccessScope::Group
        )
    })
}

/// A copy of the publisher's writer list.
fn writers_of(pp: &ZeroDdsPublisher) -> Vec<*mut ZeroDdsDataWriter> {
    pp.datawriters
        .lock()
        .map(|list| list.clone())
        .unwrap_or_default()
}

/// The group of a GROUP-scope publisher, created on first use; `None` for
/// INSTANCE and TOPIC scope.
fn writer_group(pp: &ZeroDdsPublisher) -> Option<Arc<WriterGroup>> {
    if !group_scope(pp) {
        return None;
    }
    let mut group = pp.group.lock().ok()?;
    Some(Arc::clone(
        group.get_or_insert_with(|| Arc::new(WriterGroup::new())),
    ))
}

/// The group if one was created.
fn existing_writer_group(pp: &ZeroDdsPublisher) -> Option<Arc<WriterGroup>> {
    pp.group.lock().ok()?.as_ref().map(Arc::clone)
}

/// DDSI-RTPS 2.5 §8.7.6: "Writers are not allowed to be added or removed
/// from a Publisher from the time that a coherent set begins until after
/// it ends" — for a GROUP-scope publisher, whose set spans its writers.
fn writers_may_change(pp: &ZeroDdsPublisher) -> bool {
    !(group_scope(pp) && pp.coherent_depth.lock().is_ok_and(|d| *d > 0))
}

/// Records the publisher's writers in its group, if it has one.
///
/// # Safety
/// The writers of `pp` are valid.
unsafe fn update_group_writers(pp: &ZeroDdsPublisher) {
    if let Some(group) = existing_writer_group(pp) {
        let ids: Vec<EntityId> = writers_of(pp)
            .into_iter()
            // A writer not registered with the runtime yet has no part in a
            // coherent set; it joins when it is enabled.
            // SAFETY: see fn # Safety doc.
            .filter(|dw| unsafe { (**dw).lifecycle.is_registered() })
            .map(|dw| {
                // SAFETY: see fn # Safety doc.
                unsafe { (*dw).eid }
            })
            .collect();
        group.set_writers(&ids);
    }
}

/// What a new writer takes from its publisher (as the Rust API's
/// `create_datawriter` does): under GROUP scope the group, from which it
/// draws its group sequence numbers, and the writer set its changes name;
/// inside an open coherent set the set.
///
/// # Safety
/// The writers of `pp` are valid; `dw` is in its list.
unsafe fn join_publisher(pp: &ZeroDdsPublisher, rt: &DcpsRuntime, eid: EntityId) -> bool {
    if let Some(group) = writer_group(pp) {
        // SAFETY: see fn # Safety doc.
        unsafe { update_group_writers(pp) };
        if rt.set_user_writer_group(eid, Some(group)).is_err() {
            return false;
        }
    }
    if pp.coherent_depth.lock().is_ok_and(|d| *d > 0) {
        let _ = rt.begin_user_writer_coherent_set(eid);
    }
    true
}

/// wait_for_acknowledgments(timeout_ms) — waits until all DWs are
/// acked or the timeout fires.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_wait_for_acknowledgments(
    pub_: *mut ZeroDdsPublisher,
    timeout_ms: u64,
) -> c_int {
    // SAFETY: a non-null `pub_` is a live publisher handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(pub_) } {
        return rc;
    }
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above; the dws list from
    // pub_create_datawriter (Box::into_raw), lives for the publisher lifetime.
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
            // SAFETY: dw from Box::into_raw in pub_create_datawriter.
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

/// Creates a DataWriter over the topic.
///
/// # Safety
/// `pub_`, `topic` valid; `qos` may be NULL (default = pub.default_dw_qos).
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
    // from dp_create_publisher; qos NULL-tolerant.
    unsafe {
        let pp = &*pub_;

        // The participant is required to reach the runtime.
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

        // SAFETY: `pub_` and `topic` are the non-null handles checked above.
        new_datawriter(
            pub_,
            topic,
            rt,
            qos,
            zerodds_types::TypeIdentifier::default(),
            None,
            true,
        )
    }
}

/// Creates a DataWriter over the topic that advertises its type via a
/// serialized COMPLETE `TypeObject` (F-TYPES-3 / #24).
///
/// Topic-path mirror of the flat [`crate::zerodds_writer_create_typed`]: this is
/// the variant the C#/Java bindings' generated TypeSupport calls, carrying the
/// `byte[]` TypeObject constant `idlc csharp`/`idlc java` emit. Unlike
/// [`zerodds_pub_create_datawriter`] — which advertises
/// `TypeIdentifier::default()` (== None) — it deserializes `type_object_bytes`,
/// registers the object into the shared TypeLookup registry via
/// `crate::register_type_object_from_bytes`, derives the strongly-hashed
/// `TypeIdentifier` and sets the writer slot's `type_identifier` from it
/// **before the endpoint is published** (via `register_user_writer`). There is
/// therefore no discovery window in which the endpoint is matchable while still
/// carrying `None`.
///
/// The derived identifier is byte-identical to the flat create's and to Path A's
/// `T::TYPE_IDENTIFIER` for the same TypeObject. `qos` is honored exactly as in
/// [`zerodds_pub_create_datawriter`] (PARTITION / HISTORY etc.). Returns NULL on
/// invalid arguments, a malformed TypeObject, or registration failure.
///
/// # Safety
/// Like [`zerodds_pub_create_datawriter`], plus: `type_object_bytes` must point
/// to `len` readable bytes (NULL / `len == 0` is rejected with NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_create_datawriter_typed(
    pub_: *mut ZeroDdsPublisher,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataWriterQos,
    type_object_bytes: *const u8,
    len: usize,
) -> *mut ZeroDdsDataWriter {
    // SAFETY: the unchanged public entry point forwards its caller contract.
    unsafe { create_datawriter_typed(pub_, topic, qos, type_object_bytes, len, None) }
}

/// Creates a typed writer with an explicit generated-code wire contract.
/// `is_keyed` is 0 or 1, `extensibility` is 0 (final), 1 (appendable), or
/// 2 (mutable), and `representation` is 0 (XCDR1) or 2 (XCDR2).
/// The caller must supply little-endian payload bodies in that representation.
/// The offer is fixed before discovery and cannot subsequently be changed.
/// Invalid enum values return NULL, without creating an endpoint.
///
/// # Safety
/// Same pointer/lifetime requirements as [`zerodds_pub_create_datawriter_typed`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_create_datawriter_typed_wire(
    pub_: *mut ZeroDdsPublisher,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataWriterQos,
    type_object_bytes: *const u8,
    len: usize,
    is_keyed: c_int,
    extensibility: c_int,
    representation: c_int,
) -> *mut ZeroDdsDataWriter {
    use zerodds_types::qos::ExtensibilityForRepr;
    let ext = match extensibility {
        0 => ExtensibilityForRepr::Final,
        1 => ExtensibilityForRepr::Appendable,
        2 => ExtensibilityForRepr::Mutable,
        _ => return ptr::null_mut(),
    };
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
        create_datawriter_typed(
            pub_,
            topic,
            qos,
            type_object_bytes,
            len,
            Some((is_keyed == 1, ext, repr)),
        )
    }
}

// SAFETY: `pub_` and `topic` must each be null or point to a valid
// ZeroDdsPublisher/ZeroDdsTopic; `qos` must be null or readable for
// size_of::<ZeroDdsDataWriterQos>(); `type_object_bytes` must point to `len`
// readable bytes for the caller's registration pledge; `wire` carries an
// extensibility/representation pair the caller already validated. Nulls and
// the missing runtime are refused below before any dereference.
unsafe fn create_datawriter_typed(
    pub_: *mut ZeroDdsPublisher,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataWriterQos,
    type_object_bytes: *const u8,
    len: usize,
    wire: Option<(bool, zerodds_types::qos::ExtensibilityForRepr, i16)>,
) -> *mut ZeroDdsDataWriter {
    if pub_.is_null() || topic.is_null() || type_object_bytes.is_null() || len == 0 {
        return ptr::null_mut();
    }
    // SAFETY: the contract above holds; every pointer was null-checked
    // before this block and the participant handle is re-checked below.
    unsafe {
        let pp = &*pub_;

        let dp_handle = pp.participant;
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

        let mut qos: DataWriterQos = if qos.is_null() {
            pp.default_dw_qos
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default()
        } else {
            dw_qos_from_c(qos)
        };

        if let Some((_, _, repr)) = wire {
            qos.data_representation = Some(alloc::vec![repr]);
        }

        // SAFETY: `pub_` and `topic` are the non-null handles checked above.
        let keyed = crate::type_object_keyed_from_bytes(&rt, bytes);
        new_datawriter(pub_, topic, rt, qos, type_identifier, wire, keyed)
    }
}

/// What both writer constructors share once the QoS and the type's identity
/// are decided: the handle with the id the runtime will know the writer by,
/// noted with its publisher. The writer is registered with the runtime —
/// announced and matched — when it is enabled: at once if its publisher
/// enables what it creates, at `zerodds_dw_enable` otherwise (DDS 1.4
/// §2.2.2.1.1.7, §2.2.3.20).
///
/// # Safety
/// `pub_` and `topic` are non-null, live handles.
unsafe fn new_datawriter(
    pub_: *mut ZeroDdsPublisher,
    topic: *mut ZeroDdsTopic,
    rt: Arc<DcpsRuntime>,
    qos: DataWriterQos,
    type_identifier: zerodds_types::TypeIdentifier,
    wire: Option<(bool, zerodds_types::qos::ExtensibilityForRepr, i16)>,
    keyed: bool,
) -> *mut ZeroDdsDataWriter {
    // SAFETY: the caller's contract.
    unsafe {
        let pp = &*pub_;
        if !writers_may_change(pp) {
            return ptr::null_mut();
        }
        let eid = rt.reserve_user_writer_id(wire.map_or(keyed, |w| w.0));
        let dw = Box::new(ZeroDdsDataWriter {
            instances: zerodds_dcps::instance_tracker::InstanceTracker::new(),
            publisher: pub_,
            topic,
            rt,
            eid,
            qos: Mutex::new(qos),
            partition_out: Mutex::new(Default::default()),
            lifecycle: crate::entity_ffi::Lifecycle::new(false),
        });
        let dw_ptr = Box::into_raw(dw);
        crate::entities::note_status_entity(dw_ptr, crate::entities::StatusEntityKind::DataWriter);
        (*dw_ptr).lifecycle.set_registration(
            writer_registration(dw_ptr as usize, type_identifier, wire),
            None,
        );
        if let Ok(mut list) = pp.datawriters.lock() {
            list.push(dw_ptr);
        }
        if crate::entity_ffi::publisher_autoenables(pp) && (*dw_ptr).lifecycle.enable().is_err() {
            if let Ok(mut list) = pp.datawriters.lock() {
                list.retain(|x| *x != dw_ptr);
            }
            update_group_writers(pp);
            drop(Box::from_raw(dw_ptr));
            return ptr::null_mut();
        }
        dw_ptr
    }
}

/// The registration of the writer at `dw`: the runtime gets the writer under
/// its reserved id with the QoS the writer has when it is enabled, then the
/// slot takes what it takes beside the announcement.
fn writer_registration(
    dw: usize,
    type_identifier: zerodds_types::TypeIdentifier,
    wire: Option<(bool, zerodds_types::qos::ExtensibilityForRepr, i16)>,
) -> crate::entity_ffi::Registration {
    Box::new(move |_| {
        // SAFETY: the registration runs from the handle's own lifecycle, so
        // the handle at `dw` is alive; its publisher and topic outlive it.
        let (dwr, pp, tt) = unsafe {
            let dwr = &*(dw as *const ZeroDdsDataWriter);
            (dwr, &*dwr.publisher, &*dwr.topic)
        };
        let rt = &dwr.rt;
        let eid = dwr.eid;
        let mut qos: DataWriterQos = dwr
            .qos
            .lock()
            .map(|g| g.clone())
            .map_err(|_| ZeroDdsStatus::PreconditionNotMet)?;
        if let Some((_, _, repr)) = wire {
            qos.data_representation = Some(alloc::vec![repr]);
        }
        let scoped = publisher_scoped(pp, &qos);
        let cfg = UserWriterConfig {
            topic_name: tt.name.to_string(),
            type_name: tt.type_name.to_string(),
            reliable: matches!(qos.reliability.kind, ReliabilityKind::Reliable),
            durability: qos.durability.kind,
            durability_service: qos.durability_service,
            deadline: qos.deadline.clone(),
            // The application's, as every other policy here: both were
            // left at their defaults, and DESTINATION_ORDER decides a match
            // (DDS 1.4 §2.2.3.17).
            latency_budget: qos.latency_budget,
            destination_order: qos.destination_order,
            lifespan: qos.lifespan.clone(),
            liveliness: qos.liveliness.clone(),
            ownership: qos.ownership.kind,
            ownership_strength: qos.ownership_strength.value,
            transport_priority: qos.transport_priority.value,
            // PARTITION, PRESENTATION and GROUP_DATA are the Publisher's
            // (DDS 1.4 §2.2.3.13, §2.2.3.6, §2.2.3.3); `publisher_scoped`
            // says which of the two QoS each comes from.
            presentation: scoped.presentation,
            partition: scoped.partition,
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
            group_entity_id: pp.entity_id,
            property: qos.property.clone(),
            data_tags: qos.data_tags.clone(),
            data_max_size_serialized: None,
            entity_name: None,
        };
        rt.register_user_writer_as(eid, cfg, zerodds_dcps::runtime::GroupTopicQos::default())
            .map_err(|_| ZeroDdsStatus::Error)?;
        let undo = |status| {
            rt.remove_user_writer(eid);
            status
        };
        if let Some((_, ext, _)) = wire {
            if rt.set_user_writer_wire_extensibility(eid, ext).is_err()
                || rt.lock_user_writer_representation(eid).is_err()
            {
                return Err(undo(ZeroDdsStatus::Error));
            }
        }
        {
            use zerodds_qos::HistoryKind;
            // QB-cluster: wire HISTORY keep-last DEPTH onto the runtime writer slot
            // (DDS 1.4 §2.2.3.18) so the TransientLocal retain path caps per-instance
            // retained samples. KeepAll → unbounded.
            let depth = match qos.history.kind {
                HistoryKind::KeepLast => qos.history.depth.max(1) as usize,
                HistoryKind::KeepAll => usize::MAX,
            };
            let _ = rt.set_user_writer_history_depth(eid, depth);
        }
        // DDS 1.4 §2.2.3.19 with §2.2.2.4.2.11: a write that finds no room
        // waits for an acknowledgement up to max_blocking_time.
        if rt
            .set_user_writer_resource_limits(
                eid,
                qos.resource_limits,
                qos.reliability.max_blocking_time,
            )
            .is_err()
        {
            return Err(undo(ZeroDdsStatus::Error));
        }
        // SAFETY: `pp` is the writer's live publisher; its writers are valid.
        if !unsafe { join_publisher(pp, rt, eid) } {
            return Err(undo(ZeroDdsStatus::Error));
        }
        Ok(())
    })
}

/// Deletes a DataWriter.
///
/// # Safety
/// `pub_`, `dw` valid and belonging together.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_delete_datawriter(
    pub_: *mut ZeroDdsPublisher,
    dw: *mut ZeroDdsDataWriter,
) -> c_int {
    if pub_.is_null() || dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_+dw NULL-checked above; dw from pub_create_datawriter.
    unsafe {
        if (*dw).publisher != pub_ {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        let pp = &*pub_;
        if !writers_may_change(pp) {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        if let Ok(mut list) = pp.datawriters.lock() {
            let n = list.len();
            list.retain(|x| *x != dw);
            if list.len() == n {
                return ZeroDdsStatus::BadHandle as c_int;
            }
        }
        update_group_writers(pp);
        // Drop any zero-copy SHM loan state keyed by this writer's (runtime, eid).
        #[cfg(feature = "flatdata-loan")]
        {
            let dwr = &*dw;
            crate::shm_loan_ffi::forget_writer(&dwr.rt, dwr.eid);
        }
        let _ = Box::from_raw(dw);
    }
    ZeroDdsStatus::Ok as c_int
}

/// Returns the DataWriter for a topic name, or NULL.
///
/// # Safety
/// `pub_`, `topic_name` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_lookup_datawriter(
    pub_: *mut ZeroDdsPublisher,
    topic_name: *const core::ffi::c_char,
) -> *mut ZeroDdsDataWriter {
    if pub_.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — pub_+topic_name NULL-checked above; datawriters
    // and their topic pointers from pub_create_datawriter (Box::into_raw).
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

/// Deletes all DataWriters held by the publisher.
///
/// # Safety
/// `pub_` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_delete_contained_entities(
    pub_: *mut ZeroDdsPublisher,
) -> c_int {
    if pub_.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — pub_ NULL-checked above; datawriters from
    // pub_create_datawriter (Box::into_raw).
    unsafe {
        if !writers_may_change(&*pub_) {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
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

/// Returns the topic.
///
/// # Safety
/// `dw` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_topic(dw: *mut ZeroDdsDataWriter) -> *mut ZeroDdsTopic {
    if dw.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
    unsafe { (*dw).topic }
}

/// Returns the publisher.
///
/// # Safety
/// `dw` valid.
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

/// Writes a pre-made CDR payload (the caller serializes via
/// idl codegen or DynamicData). The encap header prefix is added by the
/// runtime.
///
/// Takes no key hash. For a keyed writer whose type is registered (the
/// `*_typed`/`*_typed_wire` creates) the runtime derives the sample's
/// KeyHash from `payload` through that type, as
/// [`zerodds_dw_write_keyed`] would carry it. Any other writer's sample
/// goes out under the all-zero key (the topic's single instance, for a
/// keyless type, or an unkeyed default identity for a keyed type with no
/// registered type), and such a writer records no canonical key holder
/// for it — a later
/// [`zerodds_dw_dispose`]/[`crate::extra_ffi::zerodds_dw_unregister_instance`]
/// for it falls straight to the `PID_KEY_HASH`-only marker form (see that
/// function's own doc comment).
///
/// # Safety
/// `dw`, `payload[0..len]` valid. `handle` is `0` (HANDLE_NIL: the instance
/// comes from the key) or the handle `zerodds_dw_register_instance` returned
/// for the key of the sample; the handle of another existing instance is
/// `PreconditionNotMet`, one that names no instance `BadParameter` (DDS 1.4
/// §2.2.2.4.2.11).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_write(
    dw: *mut ZeroDdsDataWriter,
    payload: *const u8,
    len: usize,
    handle: u64,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    if dw.is_null() || (payload.is_null() && len > 0) {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+payload NULL-checked above; payload[0..len]
    // valid if len > 0 (caller pledge).
    let (dwr, bytes) = unsafe {
        let bytes: &[u8] = if len == 0 {
            &[]
        } else {
            slice::from_raw_parts(payload, len)
        };
        (&*dw, bytes)
    };
    let key = match derived_key_hash(dwr, bytes) {
        Err(status) => return status,
        Ok(key) => key,
    };
    if let Some(rc) = write_handle_refused(dwr, key, handle) {
        return rc;
    }
    match key {
        Some(key) => write_keyed(dwr, bytes, key, None),
        None => match dwr.rt.write_user_sample_borrowed(dwr.eid, bytes) {
            Ok(()) => ZeroDdsStatus::Ok as c_int,
            Err(e) => crate::write_status(&e),
        },
    }
}

/// DDS 1.4 §2.2.2.4.2.11: `handle` of a write is HANDLE_NIL (0), or "must
/// correspond to the value returned by register_instance when the instance
/// (identified by its key) was registered". `key` is the KeyHash of the
/// sample, `None` for the one instance of a topic without a key.
/// `PreconditionNotMet` for the handle of another existing instance,
/// `BadParameter` for one that names no instance.
fn write_handle_refused(
    dwr: &ZeroDdsDataWriter,
    key: Option<[u8; 16]>,
    handle: u64,
) -> Option<c_int> {
    if handle == 0 {
        return None;
    }
    let handle = zerodds_dcps::instance_handle::InstanceHandle::from_raw(handle);
    if dwr.instances.lookup(&key.unwrap_or([0u8; 16])) == Some(handle) {
        return None;
    }
    Some(if dwr.instances.get_by_handle(handle).is_some() {
        ZeroDdsStatus::PreconditionNotMet as c_int
    } else {
        ZeroDdsStatus::BadParameter as c_int
    })
}

/// The KeyHash `dwr`'s registered keyed type yields from `bytes`, the
/// sample's encoded body — `Ok(None)` for a writer whose type is unkeyed or
/// not registered, and `BadParameter` for a keyed, registered type the body
/// does not decode as.
fn derived_key_hash(dwr: &ZeroDdsDataWriter, bytes: &[u8]) -> Result<Option<[u8; 16]>, c_int> {
    if dwr.eid.entity_kind.topic_kind() != Some(zerodds_rtps::wire_types::TopicKind::WithKey) {
        return Ok(None);
    }
    match dwr.rt.writer_sample_key_material(dwr.eid, bytes) {
        Ok((key, _holder)) => Ok(Some(key)),
        Err(zerodds_dcps::DdsError::WireError { .. }) => Err(ZeroDdsStatus::BadParameter as c_int),
        Err(_) => Ok(None),
    }
}

/// A sample under its KeyHash, through this writer's instance registry —
/// the path [`zerodds_dw_write_keyed`] takes, and the one
/// [`zerodds_dw_write`] takes once it has derived the key.
fn write_keyed(
    dwr: &ZeroDdsDataWriter,
    bytes: &[u8],
    key: [u8; 16],
    source_ts: Option<zerodds_rtps::header_extension::HeTimestamp>,
) -> c_int {
    let limit = match dwr.qos.lock() {
        Ok(qos) => qos.resource_limits.max_instances,
        Err(_) => return ZeroDdsStatus::Error as c_int,
    };
    if dwr
        .instances
        .ensure_registered_limited(key, limit)
        .is_none()
    {
        return ZeroDdsStatus::OutOfResources as c_int;
    }
    match dwr
        .rt
        .write_user_sample_keyed(dwr.eid, bytes, key, source_ts)
    {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(e) => crate::write_status(&e),
    }
}

/// The 16-byte DDS KeyHash `dw`'s registered keyed type yields from the
/// encoded sample `payload[0..len]` — the key [`zerodds_dw_write`] writes
/// the same bytes under — into `out_key_hash[0..16]`.
///
/// For a binding that holds the sample but no key function of its own:
/// the C++ PSM's `DataWriter<T>::dispose_instance`, `register_instance`,
/// `lookup_instance` and `unregister_instance` encode the sample and ask
/// this, so the lifecycle call names the instance the write did.
///
/// `PreconditionNotMet` when the writer's type is not registered as a keyed
/// `CompleteTypeObject`; `BadParameter` when the body does not decode as it.
///
/// # Safety
/// `dw` valid; `payload[0..len]` readable when `len > 0`;
/// `out_key_hash[0..16]` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_sample_key_hash(
    dw: *mut ZeroDdsDataWriter,
    payload: *const u8,
    len: usize,
    out_key_hash: *mut u8,
) -> c_int {
    if dw.is_null() || out_key_hash.is_null() || (payload.is_null() && len > 0) {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: pointers checked above; the caller pledges a live writer and
    // `len` readable bytes.
    let (dwr, bytes) = unsafe {
        let bytes: &[u8] = if len == 0 {
            &[]
        } else {
            slice::from_raw_parts(payload, len)
        };
        (&*dw, bytes)
    };
    match dwr.rt.writer_sample_key_material(dwr.eid, bytes) {
        Ok((key, _holder)) => {
            // SAFETY: `out_key_hash` is 16 writable bytes per the caller pledge.
            unsafe { ptr::copy_nonoverlapping(key.as_ptr(), out_key_hash, 16) };
            ZeroDdsStatus::Ok as c_int
        }
        Err(zerodds_dcps::DdsError::WireError { .. }) => ZeroDdsStatus::BadParameter as c_int,
        Err(_) => ZeroDdsStatus::PreconditionNotMet as c_int,
    }
}

/// Writes a pre-serialized sample with its actual 16-byte DDS KeyHash.
/// No key is inferred from the complete payload or a truncated instance handle.
/// The runtime receives the key for keyed history and sample delivery.
///
/// This is the only C-FFI write function that takes an explicit key hash
/// (`zerodds_dw_write`/`zerodds_dw_write_w_timestamp`/
/// [`crate::zerodds_writer_write`] derive it from the body for a keyed
/// writer whose type is registered, and write under the all-zero key
/// otherwise). The runtime also uses this call to remember the instance's
/// canonical key holder from `payload`'s body (`UserWriterSlot::
/// instance_key_holders`, once per key hash, decoded through the
/// TypeLookup registry if this writer's type resolves there) — see
/// [`zerodds_dw_dispose`] for what that buys a later dispose/unregister of
/// the same instance.
///
/// # Safety
/// `dw` must be valid, `payload[0..len]` readable when `len > 0`, and
/// `key_hash[0..key_len]` readable. A null payload is allowed only for zero bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_write_keyed(
    dw: *mut ZeroDdsDataWriter,
    payload: *const u8,
    len: usize,
    key_hash: *const u8,
    key_len: usize,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    if dw.is_null() || (payload.is_null() && len > 0) || key_hash.is_null() || key_len != 16 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: pointers and lengths were checked above; the caller pledges
    // readable storage for each nonempty slice and a live writer handle.
    let (dwr, bytes, key) = unsafe {
        let bytes = if len == 0 {
            &[]
        } else {
            slice::from_raw_parts(payload, len)
        };
        let mut key = [0u8; 16];
        key.copy_from_slice(slice::from_raw_parts(key_hash, 16));
        (&*dw, bytes, key)
    };
    write_keyed(dwr, bytes, key, None)
}

/// Writes with an explicit source timestamp (Spec §2.2.2.4.2.16
/// `write_w_timestamp`), otherwise identical to [`zerodds_dw_write`] --
/// `handle` follows the rule of that function, and the key is derived from
/// the body the same way. `ts_sec`/
/// `ts_nanosec` become the sample's actual SOURCE_TIMESTAMP on the wire
/// INFO_TS and the same-runtime intra-process dispatch, not just local
/// bookkeeping: this used to forward straight to `zerodds_dw_write`,
/// silently discarding both.
///
/// # Safety
/// Like `zerodds_dw_write`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_write_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    payload: *const u8,
    len: usize,
    handle: u64,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    // A nanosecond field of one second or more is not a time (DDS 1.4
    // §2.2.2.4.2.16 answers BAD_PARAMETER for an invalid timestamp).
    if dw.is_null() || (payload.is_null() && len > 0) || ts_nanosec >= 1_000_000_000 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+payload NULL-checked above; payload[0..len]
    // valid if len > 0 (caller pledge).
    let (dwr, bytes) = unsafe {
        let bytes: &[u8] = if len == 0 {
            &[]
        } else {
            slice::from_raw_parts(payload, len)
        };
        (&*dw, bytes)
    };
    let ts = zerodds_dcps::time::time_to_he_timestamp(zerodds_dcps::Time::new(ts_sec, ts_nanosec));
    let key = match derived_key_hash(dwr, bytes) {
        Err(status) => return status,
        Ok(key) => key,
    };
    if let Some(rc) = write_handle_refused(dwr, key, handle) {
        return rc;
    }
    match key {
        Some(key) => write_keyed(dwr, bytes, key, Some(ts)),
        None => match dwr.rt.write_user_sample_stamped(dwr.eid, bytes, ts) {
            Ok(()) => ZeroDdsStatus::Ok as c_int,
            Err(e) => crate::write_status(&e),
        },
    }
}

/// Dispose: emits a DISPOSED lifecycle marker for the instance.
///
/// This call itself hands the runtime only `key_hash`, no key value — but
/// unlike [`crate::zerodds_writer_dispose`] (which truly never has one),
/// `write_user_lifecycle` (`crates/dcps/src/runtime.rs`) falls back to the
/// key holder THIS writer remembered from `key_hash`'s first
/// [`zerodds_dw_write_keyed`] call, if any, and this writer's type resolves
/// as a `CompleteTypeObject` in the TypeLookup registry: the DISPOSED
/// marker then goes out with the full `serializedKey` (RTPS 2.5 §9.4.2.11
/// K flag set), the only dispose form Eclipse Cyclone 11.0.1, Fast DDS
/// 3.6.1 and OpenDDS 3.35 were all measured to act on
/// (`Test_FinalInstanceState_1`). An instance this writer never wrote via
/// `zerodds_dw_write_keyed` (only `zerodds_dw_write`/
/// `zerodds_dw_write_w_timestamp`, or `zerodds_dw_write_keyed` under a
/// different key hash), or whose type does not resolve there, still gets
/// the `PID_KEY_HASH`-only marker instead — RTI Connext 7.7.0 and CoreDX
/// 6.15.3 accept that form too, so this is a strictly wider compatibility
/// set than the marker used to be, never narrower.
///
/// # Safety
/// `dw` valid; `key_hash[0..16]` readable. `handle` informational.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_dispose(
    dw: *mut ZeroDdsDataWriter,
    key_hash: *const u8,
    handle: u64,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    // SAFETY: the caller's contract.
    unsafe { dispose_at(dw, key_hash, handle, None) }
}

/// The key a writer tracks an instance under. DDS 1.4 §2.2.1.2.2: a topic
/// without a key has one instance, so a writer of such a topic (entityKind
/// `NO_KEY`) names it by the all-zero key whatever key the caller passes.
pub(crate) fn instance_key_of(eid: EntityId, key: [u8; 16]) -> [u8; 16] {
    if eid.entity_kind == zerodds_rtps::wire_types::EntityKind::UserWriterNoKey {
        [0u8; 16]
    } else {
        key
    }
}

/// [`zerodds_dw_dispose`] with the marker's source timestamp (DDS 1.4
/// §2.2.2.4.2.11 `dispose_w_timestamp`).
///
/// # Safety
/// As `zerodds_dw_dispose`.
pub(crate) unsafe fn dispose_at(
    dw: *mut ZeroDdsDataWriter,
    key_hash: *const u8,
    _handle: u64,
    source_timestamp: Option<zerodds_rtps::header_extension::HeTimestamp>,
) -> c_int {
    if dw.is_null() || key_hash.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+key_hash NULL-checked above; key_hash[0..16]
    // valid (caller pledge).
    let (rt, eid, k) = unsafe {
        let dwr = &*dw;
        let raw = slice::from_raw_parts(key_hash, 16);
        let mut k = [0u8; 16];
        k.copy_from_slice(raw);
        (dwr.rt.clone(), dwr.eid, instance_key_of(dwr.eid, k))
    };
    // Status bits: DISPOSED per RTPS inline-QoS status info §9.6.4.10.
    const STATUS_DISPOSED: u32 = 0x0000_0001;
    match rt.write_user_lifecycle(eid, k, None, STATUS_DISPOSED, source_timestamp) {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(e) => crate::write_status(&e),
    }
}

/// Waits until all currently outstanding writes are acked or a timeout.
///
/// # Safety
/// `dw` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_wait_for_acknowledgments(
    dw: *mut ZeroDdsDataWriter,
    timeout_ms: u64,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
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

/// Manually asserts liveliness for this writer.
///
/// # Safety
/// `dw` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_assert_liveliness(dw: *mut ZeroDdsDataWriter) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — dw NULL-checked above.
    unsafe { (*dw).rt.assert_liveliness() };
    ZeroDdsStatus::Ok as c_int
}

/// Waits until at least `min` matched subscriptions exist or a timeout.
///
/// # Safety
/// `dw` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_wait_for_matched(
    dw: *mut ZeroDdsDataWriter,
    min: i32,
    timeout_ms: u64,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle (DDS 1.4 §2.2.2.1.1.7).
    if let Some(rc) = unsafe { crate::entity_ffi::not_enabled(dw) } {
        return rc;
    }
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

/// OfferedIncompatibleQosStatus (Spec §2.2.4.1, without a per-policy list —
/// `last_policy_id` covers the mandatory path).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ZeroDdsOfferedIncompatibleQosStatus {
    pub total_count: i32,
    pub total_count_change: i32,
    pub last_policy_id: u32,
}

/// `get_liveliness_lost_status` (DDS 1.4 §2.2.2.4.2). Reading resets the
/// change field (§2.2.4.1).
///
/// # Safety
/// `dw` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_liveliness_lost_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsLivelinessLostStatus,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle; the status of a disabled
    // entity is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dw, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above.
    unsafe {
        let dwr = &*dw;
        let st = dwr
            .rt
            .take_user_writer_liveliness_lost_status(dwr.eid)
            .unwrap_or_default();
        *out = ZeroDdsLivelinessLostStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_offered_deadline_missed_status` (DDS 1.4 §2.2.2.4.2). Reading
/// resets the change field.
///
/// # Safety
/// `dw` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_offered_deadline_missed_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsOfferedDeadlineMissedStatus,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle; the status of a disabled
    // entity is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dw, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above.
    unsafe {
        let dwr = &*dw;
        let st = dwr
            .rt
            .take_user_writer_offered_deadline_missed_status(dwr.eid)
            .unwrap_or_default();
        *out = ZeroDdsOfferedDeadlineMissedStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
            // The runtime knows the instance by key hash; the writer's
            // tracker holds its handle.
            last_instance_handle: dwr
                .rt
                .user_writer_missed_deadline_instance(dwr.eid)
                .and_then(|key| dwr.instances.lookup(&key))
                .unwrap_or(st.last_instance_handle)
                .as_raw(),
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_offered_incompatible_qos_status` (DDS 1.4 §2.2.2.4.2). Reading
/// resets the change field.
///
/// # Safety
/// `dw` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_offered_incompatible_qos_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsOfferedIncompatibleQosStatus,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle; the status of a disabled
    // entity is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dw, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above.
    unsafe {
        let dwr = &*dw;
        let st = dwr
            .rt
            .take_user_writer_offered_incompatible_qos_status(dwr.eid)
            .unwrap_or_default();
        *out = ZeroDdsOfferedIncompatibleQosStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
            last_policy_id: st.last_policy_id,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// `get_publication_matched_status` (DDS 1.4 §2.2.2.4.2): `total_count`
/// counts every reader that ever matched, `current_count` those matched
/// now, and the last handle is the reader that last joined or left.
/// Reading resets the change fields. It wrote the matched count into both
/// counts, 0 into both changes and HANDLE_NIL into the handle.
///
/// # Safety
/// `dw` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_publication_matched_status(
    dw: *mut ZeroDdsDataWriter,
    out: *mut ZeroDdsPublicationMatchedStatus,
) -> c_int {
    // SAFETY: a non-null `dw` is a live DataWriter handle; the status of a disabled
    // entity is untriggered (DDS 1.4 §2.2.2.1.1.7).
    if unsafe { crate::entity_ffi::untriggered_if_disabled(dw, out) } {
        return ZeroDdsStatus::Ok as c_int;
    }
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — dw+out NULL-checked above.
    unsafe {
        let dwr = &*dw;
        let st = dwr
            .rt
            .take_user_writer_publication_matched_status(dwr.eid)
            .unwrap_or_default();
        *out = ZeroDdsPublicationMatchedStatus {
            total_count: st.total_count,
            total_count_change: st.total_count_change,
            current_count: st.current_count,
            current_count_change: st.current_count_change,
            last_subscription_handle: st.last_subscription_handle.as_raw(),
        };
    }
    ZeroDdsStatus::Ok as c_int
}

// Helper used by tests/internals; type-suppress `DurabilityKind` import warning.
#[allow(dead_code)]
fn _suppress_durability_warning() -> DurabilityKind {
    DurabilityKind::default()
}

/// A writer's PARTITION, PRESENTATION and GROUP_DATA — the three policies
/// DDS 1.4 places on the Publisher (§2.2.3.13, §2.2.3.6, §2.2.3.3) and not
/// on the DataWriter.
pub(crate) struct PublisherScoped {
    pub(crate) partition: Vec<String>,
    pub(crate) presentation: zerodds_qos::PresentationQosPolicy,
    pub(crate) group_data: Vec<u8>,
}

/// The Publisher's PARTITION, PRESENTATION and GROUP_DATA for a writer it
/// creates. The C ABI's `ZeroDdsDataWriterQos` carries a partition and a
/// group data of its own; where the caller filled one in, that one is used,
/// so a caller that already copied the publisher's onto the writer — the
/// TypeScript binding does — gets what it asked for. The endpoint used to
/// take only the writer's, so a partition set where DDS puts it, on the
/// Publisher, gated nothing: `A` on the publisher matched `B` on the
/// subscriber (measured in zerodds-examples idl-conformance/_qos/cpp).
pub(crate) fn publisher_scoped(pp: &ZeroDdsPublisher, qos: &DataWriterQos) -> PublisherScoped {
    let publisher = pp.qos.lock().map(|g| g.clone()).unwrap_or_default();
    PublisherScoped {
        partition: if qos.partition.names.is_empty() {
            publisher.partition.names
        } else {
            qos.partition.names.clone()
        },
        presentation: publisher.presentation,
        group_data: if qos.group_data.value.is_empty() {
            publisher.group_data.value
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
        zerodds_dp_create_publisher, zerodds_dp_create_topic, zerodds_dp_delete_contained_entities,
    };

    /// DDS 1.4 §2.2.1.2.2: a writer of a topic without a key (entityKind
    /// `NO_KEY`) names its one instance by the all-zero key, whatever key the
    /// caller passes; a keyed writer keeps the key.
    #[test]
    fn a_keyless_writer_names_its_one_instance_by_the_zero_key() {
        let keyless = EntityId::user_writer_no_key([1, 2, 3]);
        let keyed = EntityId::user_writer_with_key([1, 2, 3]);
        assert_eq!(instance_key_of(keyless, [7u8; 16]), [0u8; 16]);
        assert_eq!(instance_key_of(keyed, [7u8; 16]), [7u8; 16]);
    }

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
        // SAFETY: f from dpf_get_instance, n+tn statically valid.
        unsafe {
            let p = zerodds_dpf_create_participant(f, domain, ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            (p, pubh, t)
        }
    }

    fn cleanup(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: p from mk_pub_topic; f static.
        unsafe {
            zerodds_dp_delete_contained_entities(p);
            zerodds_dpf_delete_participant(f, p);
        }
    }

    /// DDS 1.4 §2.2.4.1 through C: `total_count` counts every reader that
    /// ever matched, `current_count` follows an unmatch, a read takes the
    /// changes, and the last handle names the reader. It wrote the match
    /// count into both counts and 0 into the changes and the handle.
    #[test]
    fn publication_matched_through_c_is_cumulative_and_read_once() {
        use crate::participant_ffi::zerodds_dp_create_subscriber;
        use crate::subscriber_ffi::{zerodds_sub_create_datareader, zerodds_sub_delete_datareader};
        let (participant, publisher, topic) = mk_pub_topic(38);
        let ok = ZeroDdsStatus::Ok as c_int;
        let mut st = ZeroDdsPublicationMatchedStatus::default();
        // SAFETY: every handle comes from a create call on `participant`.
        unsafe {
            let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
            let subscriber = zerodds_dp_create_subscriber(participant, ptr::null());
            let first = zerodds_sub_create_datareader(subscriber, topic, ptr::null());
            assert_eq!(
                zerodds_dw_get_publication_matched_status(writer, &mut st),
                ok
            );
            assert_eq!(
                (
                    st.total_count,
                    st.total_count_change,
                    st.current_count,
                    st.current_count_change
                ),
                (1, 1, 1, 1)
            );
            assert_ne!(st.last_subscription_handle, 0);
            let first_handle = st.last_subscription_handle;
            assert_eq!(
                zerodds_dw_get_publication_matched_status(writer, &mut st),
                ok
            );
            assert_eq!((st.total_count_change, st.current_count_change), (0, 0));

            assert_eq!(zerodds_sub_delete_datareader(subscriber, first), ok);
            assert_eq!(
                zerodds_dw_get_publication_matched_status(writer, &mut st),
                ok
            );
            assert_eq!(
                (st.total_count, st.current_count, st.current_count_change),
                (1, 0, -1)
            );
            assert_eq!(st.last_subscription_handle, first_handle);

            let _second = zerodds_sub_create_datareader(subscriber, topic, ptr::null());
            assert_eq!(
                zerodds_dw_get_publication_matched_status(writer, &mut st),
                ok
            );
            assert_eq!(
                (st.total_count, st.total_count_change, st.current_count),
                (2, 1, 1)
            );
            assert_ne!(st.last_subscription_handle, first_handle);
        }
        cleanup(participant);
    }

    #[test]
    fn keyed_write_refuses_invalid_key_lengths_and_null_storage() {
        let (participant, publisher, topic) = mk_pub_topic(50);
        // SAFETY: handles from mk_pub_topic and slices below are live. Invalid
        // lengths/null combinations must be rejected before dereferencing.
        unsafe {
            let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
            assert!(!writer.is_null());
            let key = [7u8; 17];
            for length in [0, 15, 17] {
                assert_eq!(
                    zerodds_dw_write_keyed(writer, ptr::null(), 0, key.as_ptr(), length),
                    ZeroDdsStatus::BadParameter as c_int,
                );
            }
            assert_eq!(
                zerodds_dw_write_keyed(writer, ptr::null(), 0, ptr::null(), 16),
                ZeroDdsStatus::BadParameter as c_int,
            );
            assert_eq!(
                zerodds_dw_write_keyed(writer, ptr::null(), 1, key.as_ptr(), 16),
                ZeroDdsStatus::BadParameter as c_int,
            );
            assert_eq!(
                zerodds_dw_write_keyed(ptr::null_mut(), ptr::null(), 0, key.as_ptr(), 16),
                ZeroDdsStatus::BadParameter as c_int,
            );
            assert_eq!(
                zerodds_pub_delete_datawriter(publisher, writer),
                ZeroDdsStatus::Ok as c_int
            );
        }
        cleanup(participant);
    }

    #[test]
    fn create_delete_datawriter_roundtrip() {
        let (p, pubh, t) = mk_pub_topic(41);
        assert!(!p.is_null() && !pubh.is_null() && !t.is_null());
        // SAFETY: pubh+t from mk.
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
        // SAFETY: pubh+t from mk; n static.
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
        // SAFETY: pubh+t from mk; n static.
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
        // SAFETY: pubh+t from mk; payload stack-local.
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
        // SAFETY: pubh+t from mk.
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
        // SAFETY: pubh from mk.
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

    /// F-TYPES-3 / #24: the topic-based typed create
    /// (`zerodds_pub_create_datawriter_typed` /
    /// `zerodds_sub_create_datareader_typed` — the path the C#/Java bindings
    /// use) deserializes the emitted COMPLETE TypeObject bytes, registers the
    /// object BEFORE the endpoint is published, and derives the identifier
    /// byte-identical to `StructBuilder::build_complete`. Proven by the object
    /// landing in the runtime registry under the hash computed from the
    /// builder's own COMPLETE TypeObject.
    #[test]
    fn pub_sub_create_typed_registers_before_publish() {
        use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
        use zerodds_types::type_object::{CompleteTypeObject, TypeObject};
        use zerodds_types::{PrimitiveKind, TypeIdentifier, compute_complete_hash};

        // Path A: the SAME builder the flat typed-create test uses.
        let cs = TypeObjectBuilder::struct_type("Sensor")
            .extensibility(Extensibility::Appendable)
            .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                m.key()
            })
            .member(
                "value",
                TypeIdentifier::Primitive(PrimitiveKind::Float64),
                |m| m,
            )
            .build_complete();
        let to = TypeObject::Complete(CompleteTypeObject::Struct(cs));
        let TypeObject::Complete(cto) = &to else {
            panic!("built a Complete TypeObject")
        };
        let expected_hash = compute_complete_hash(cto).unwrap();
        let bytes = to.to_bytes_le().unwrap();

        let (p, pubh, t) = mk_pub_topic(48);
        // SAFETY: p/pubh/t from mk; bytes is a valid slice.
        unsafe {
            let subh = crate::participant_ffi::zerodds_dp_create_subscriber(p, ptr::null());
            assert!(!subh.is_null());

            let dw = zerodds_pub_create_datawriter_typed(
                pubh,
                t,
                ptr::null(),
                bytes.as_ptr(),
                bytes.len(),
            );
            assert!(!dw.is_null(), "pub_create_datawriter_typed returned NULL");

            let dr = crate::subscriber_ffi::zerodds_sub_create_datareader_typed(
                subh,
                t,
                ptr::null(),
                bytes.as_ptr(),
                bytes.len(),
            );
            assert!(!dr.is_null(), "sub_create_datareader_typed returned NULL");

            // The COMPLETE TypeObject is in the runtime registry, keyed by the
            // hash the StructBuilder computes — deserialize+register+derive is
            // byte-identical to build_complete.
            {
                let rt = (*dw).rt.clone();
                let server = rt.type_lookup_server.lock().unwrap();
                assert!(
                    server.registry.get_complete(&expected_hash).is_some(),
                    "typed pub/sub create did not register the TypeObject under its build_complete hash"
                );
            }

            // A malformed TypeObject is rejected — never a None-identifier slot.
            let garbage: [u8; 3] = [0, 1, 2];
            assert!(
                zerodds_pub_create_datawriter_typed(
                    pubh,
                    t,
                    ptr::null(),
                    garbage.as_ptr(),
                    garbage.len(),
                )
                .is_null()
            );
            assert!(
                zerodds_pub_create_datawriter_typed(pubh, t, ptr::null(), ptr::null(), 0).is_null()
            );

            let _ = zerodds_pub_delete_datawriter(pubh, dw);
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
        // SAFETY: pubh+t from mk; status slots stack-local.
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

    #[test]
    fn typed_wire_creation_pins_representation_and_endpoint_kind() {
        use zerodds_rtps::wire_types::TopicKind;
        use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
        use zerodds_types::type_object::{CompleteTypeObject, TypeObject};

        let object = TypeObject::Complete(CompleteTypeObject::Struct(
            TypeObjectBuilder::struct_type("PubType")
                .extensibility(Extensibility::Appendable)
                .build_complete(),
        ));
        let bytes = object.to_bytes_le().unwrap();
        let (p, pubh, topic) = mk_pub_topic(49);
        // SAFETY: all entity pointers and bytes remain live until cleanup.
        unsafe {
            let sub = crate::participant_ffi::zerodds_dp_create_subscriber(p, ptr::null());
            for representation in [0, 2] {
                let writer = zerodds_pub_create_datawriter_typed_wire(
                    pubh,
                    topic,
                    ptr::null(),
                    bytes.as_ptr(),
                    bytes.len(),
                    0,
                    1,
                    representation,
                );
                assert!(!writer.is_null());
                let writer = &*writer;
                assert_eq!(writer.eid.entity_kind.topic_kind(), Some(TopicKind::NoKey));
                assert_eq!(
                    writer.rt.user_writer_offer_first(writer.eid),
                    Some(representation as i16)
                );
                assert_eq!(
                    writer.qos.lock().unwrap().data_representation,
                    Some(alloc::vec![representation as i16])
                );
                assert!(
                    writer
                        .rt
                        .set_user_writer_data_rep_override(writer.eid, Some(alloc::vec![2]))
                        .is_err()
                );
                let reader = crate::subscriber_ffi::zerodds_sub_create_datareader_typed_wire(
                    sub,
                    topic,
                    ptr::null(),
                    bytes.as_ptr(),
                    bytes.len(),
                    0,
                    representation,
                );
                assert!(!reader.is_null());
                assert_eq!(
                    (*reader).eid.entity_kind.topic_kind(),
                    Some(TopicKind::NoKey)
                );
                assert_eq!(
                    (*reader).qos.lock().unwrap().data_representation,
                    Some(alloc::vec![representation as i16])
                );
            }
            // In particular, the managed sample-info tag 1 is NOT native XCDR2.
            for (keyed, ext, repr) in [(0, 1, 1), (2, 1, 2), (0, 3, 2), (0, 1, -1)] {
                assert!(
                    zerodds_pub_create_datawriter_typed_wire(
                        pubh,
                        topic,
                        ptr::null(),
                        bytes.as_ptr(),
                        bytes.len(),
                        keyed,
                        ext,
                        repr,
                    )
                    .is_null()
                );
            }
            for (keyed, repr) in [(0, 1), (2, 2), (0, -1)] {
                assert!(
                    crate::subscriber_ffi::zerodds_sub_create_datareader_typed_wire(
                        sub,
                        topic,
                        ptr::null(),
                        bytes.as_ptr(),
                        bytes.len(),
                        keyed,
                        repr,
                    )
                    .is_null()
                );
            }
        }
        cleanup(p);
    }
}
