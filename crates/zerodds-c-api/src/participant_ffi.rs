// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DomainParticipant C-FFI (Spec §2.2.2.2.1 + DDS-PSM-Cxx §7.5.2).
//!
//! Implements the C surface of the DomainParticipant operations from
//! `docs/specs/zerodds-c-api-1.0.md` §2.2. QoS pointers in
//! the RC1 surface accept NULL (default) or are backed definitively in `qos.rs`
//! with `#[repr(C)]` layouts — the function signatures here
//! already anchor the final ABI contract.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::{c_char, c_int};
use core::ptr;
use core::slice;
use std::ffi::CStr;
use std::sync::Mutex;

use zerodds_dcps::qos::{DataReaderQos, DataWriterQos, PublisherQos, SubscriberQos, TopicQos};

use crate::ZeroDdsStatus;
use crate::entities::{
    ZeroDdsContentFilteredTopic, ZeroDdsDomainParticipant, ZeroDdsPublisher, ZeroDdsSubscriber,
    ZeroDdsTopic,
};
use crate::ffi_helpers::copy_list_out;
use crate::qos_ffi::{
    ZeroDdsPublisherQos, ZeroDdsSubscriberQos, ZeroDdsTime, ZeroDdsTopicQos, pub_qos_from_c,
    sub_qos_from_c, topic_qos_from_c,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Safe cast `*const c_char` → `&str`. Returns `Err` on NULL or
/// if not UTF-8.
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe fn cstr_to_str<'a>(p: *const c_char) -> Result<&'a str, ()> {
    if p.is_null() {
        return Err(());
    }
    // SAFETY: NULL check above + caller contract for C-string termination.
    let cs = unsafe { CStr::from_ptr(p) };
    cs.to_str().map_err(|_| ())
}

// ---------------------------------------------------------------------------
// Topic
// ---------------------------------------------------------------------------

/// Creates a topic. Returns NULL on NULL arguments or a topic-name
/// conflict with another type.
///
/// # Safety
/// `p`, `name`, `type_name` must be valid. `qos` may be NULL
/// (default from `default_topic_qos`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_create_topic(
    p: *mut ZeroDdsDomainParticipant,
    name: *const c_char,
    type_name: *const c_char,
    qos: *const ZeroDdsTopicQos,
) -> *mut ZeroDdsTopic {
    if p.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; name/type_name/qos
    // per caller pledge; the topics list holds only pointers from Box::into_raw.
    unsafe {
        let pp = &*p;
        let name_s = match cstr_to_str(name) {
            Ok(s) if !s.is_empty() => s.to_string(),
            _ => return ptr::null_mut(),
        };
        let type_s = match cstr_to_str(type_name) {
            Ok(s) if !s.is_empty() => s.to_string(),
            _ => return ptr::null_mut(),
        };

        // QoS path: caller-supplied if non-NULL, otherwise the participant default.
        let qos: TopicQos = if qos.is_null() {
            pp.default_topic_qos
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default()
        } else {
            topic_qos_from_c(qos)
        };

        // DDS-XTypes 1.3 §7.6.4.1: "The Service may instantiate multiple
        // objects of the same name, provided that all of them represent
        // type-based subsets of 'the same' network topic": they "must have
        // consistent QoS with one another", and the first object's QoS is
        // the one the others are held to. A second type under the name is
        // therefore another topic when its QoS equals the first's, and a
        // conflict when it does not.
        if let Ok(list) = pp.topics.lock() {
            let mut first_of_the_name = None;
            let mut type_is_known = false;
            for &existing in list.iter() {
                if existing.is_null() {
                    continue;
                }
                let t = &*existing;
                if t.name == name_s {
                    first_of_the_name.get_or_insert(existing);
                    type_is_known |= t.type_name == type_s;
                }
            }
            if let (Some(first), false) = (first_of_the_name, type_is_known) {
                let consistent = (*first).qos.lock().is_ok_and(|first_qos| *first_qos == qos);
                if !consistent {
                    return ptr::null_mut();
                }
            }
        }

        let topic = Box::new(ZeroDdsTopic {
            participant: p,
            name: name_s,
            type_name: type_s,
            qos: Mutex::new(qos),
            support: Mutex::new(None),
            inconsistent_topic_read: core::sync::atomic::AtomicI32::new(0),
            lifecycle: crate::entity_ffi::Lifecycle::new(
                crate::entity_ffi::participant_autoenables(pp),
            ),
        });
        let t = Box::into_raw(topic);
        crate::entities::note_status_entity(t, crate::entities::StatusEntityKind::Topic);
        if let Ok(mut list) = pp.topics.lock() {
            list.push(t);
        }
        t
    }
}

/// Deletes a topic (DDS 1.4 §2.2.2.2.1.6 `delete_topic`).
///
/// "The deletion of a Topic is not allowed if there are any existing
/// DataReader, DataWriter, ContentFilteredTopic, or MultiTopic objects that
/// are using the Topic… it will return PRECONDITION_NOT_MET. The
/// delete_topic operation must be called on the same DomainParticipant
/// object used to create the Topic. If delete_topic is called on a different
/// DomainParticipant, the operation will have no effect and it will return
/// PRECONDITION_NOT_MET."
///
/// Both sentences are return codes here: a topic another participant owns,
/// and a topic a live writer, reader or ContentFilteredTopic still uses, are
/// both `ZERODDS_PRECONDITION_NOT_MET`. The users are counted over the
/// endpoints of this participant's publishers and subscribers — a DataReader
/// created on a ContentFilteredTopic carries that topic's pointer — so the
/// counts follow the same objects the delete operations take out of those
/// lists.
///
/// The Rust participant behind this handle owns the SEDP topics writer and
/// is asked for the disposal: it removes the name from that participant's
/// typed topic registry and disposes the announcement there. A `ZeroDdsTopic`
/// is a name and a type name, not a typed topic, so for a topic this FFI
/// created there is no registry entry and the disposal is the whole of the
/// call's effect; the ALREADY_DELETED the Rust call reports for exactly that
/// is not an error here.
///
/// # Safety
/// `p` and `t` must be valid handles and belong together.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_topic(
    p: *mut ZeroDdsDomainParticipant,
    t: *mut ZeroDdsTopic,
) -> c_int {
    if p.is_null() || t.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p+t NULL-checked above; t from create_topic Box::into_raw.
    unsafe {
        let pp = &*p;
        if (*t).participant != p {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        // §2.2.2.2.1.6: a live user of the topic forbids the deletion.
        if let Some(user) = first_topic_user(pp, t) {
            return user;
        }
        // The Rust participant owns the SEDP topics writer; hand it the name.
        match pp.dp.delete_topic_by_name(&(*t).name) {
            Ok(()) => {}
            Err(zerodds_dcps::DdsError::AlreadyDeleted) => {}
            Err(zerodds_dcps::DdsError::PreconditionNotMet { .. }) => {
                return ZeroDdsStatus::PreconditionNotMet as c_int;
            }
            Err(_) => return ZeroDdsStatus::Error as c_int,
        }
        let mut known = false;
        if let Ok(mut list) = pp.topics.lock() {
            let before = list.len();
            list.retain(|x| *x != t);
            known = list.len() != before;
        }
        if !known {
            return ZeroDdsStatus::BadHandle as c_int;
        }
        let _ = Box::from_raw(t);
    }
    ZeroDdsStatus::Ok as c_int
}

/// The first live DataWriter, DataReader or ContentFilteredTopic of `p` that
/// uses `topic`, as the return code the DDS 1.4 §2.2.2.2.1.6 precondition
/// reports for it: `Error` when a list cannot be read, because a count that
/// could not be taken is not a count of zero.
fn first_topic_user(p: &ZeroDdsDomainParticipant, topic: *mut ZeroDdsTopic) -> Option<c_int> {
    let publishers = match p.publishers.lock() {
        Ok(v) => v,
        Err(_) => return Some(ZeroDdsStatus::Error as c_int),
    };
    for &pub_ in publishers.iter() {
        if pub_.is_null() {
            continue;
        }
        // SAFETY: every pointer in `publishers` came from Box::into_raw in
        // zerodds_dp_create_publisher and is only removed by a delete.
        let pb = unsafe { &*pub_ };
        let writers = match pb.datawriters.lock() {
            Ok(v) => v,
            Err(_) => return Some(ZeroDdsStatus::Error as c_int),
        };
        if writers.iter().any(|&dw| {
            // SAFETY: a non-null entry of a publisher's list is a writer
            // from zerodds_pub_create_datawriter, removed only by its delete.
            !dw.is_null() && unsafe { &*dw }.topic == topic
        }) {
            return Some(ZeroDdsStatus::PreconditionNotMet as c_int);
        }
    }
    let subscribers = match p.subscribers.lock() {
        Ok(v) => v,
        Err(_) => return Some(ZeroDdsStatus::Error as c_int),
    };
    for &sub in subscribers.iter() {
        if sub.is_null() {
            continue;
        }
        // SAFETY: as above, for the subscriber list.
        let sb = unsafe { &*sub };
        let readers = match sb.datareaders.lock() {
            Ok(v) => v,
            Err(_) => return Some(ZeroDdsStatus::Error as c_int),
        };
        if readers.iter().any(|&dr| {
            // SAFETY: as above, for a subscriber's readers.
            !dr.is_null() && unsafe { &*dr }.topic == topic
        }) {
            return Some(ZeroDdsStatus::PreconditionNotMet as c_int);
        }
    }
    let cfts = match p.content_filtered_topics.lock() {
        Ok(v) => v,
        Err(_) => return Some(ZeroDdsStatus::Error as c_int),
    };
    if cfts.iter().any(|&cft| {
        // SAFETY: a non-null entry is a ContentFilteredTopic from its create
        // call, removed only by its delete.
        !cft.is_null() && unsafe { &*cft }.related_topic == topic
    }) {
        return Some(ZeroDdsStatus::PreconditionNotMet as c_int);
    }
    None
}

/// Finds an already-created topic by name. Returns NULL if none.
///
/// # Safety
/// `p`, `name` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_find_topic(
    p: *mut ZeroDdsDomainParticipant,
    name: *const c_char,
) -> *mut ZeroDdsTopic {
    if p.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; name per caller pledge;
    // the topics list holds only valid box pointers.
    unsafe {
        let pp = &*p;
        let name_s = match cstr_to_str(name) {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        };
        if let Ok(list) = pp.topics.lock() {
            for &t in list.iter() {
                if t.is_null() {
                    continue;
                }
                if (*t).name == name_s {
                    return t;
                }
            }
        }
    }
    ptr::null_mut()
}

/// `find_topic(name, timeout)` (DDS 1.4 §2.2.2.2.1.11): the topic of that
/// name if this participant has it, otherwise waits until it is created, by
/// another thread for one, or until `timeout_ms` milliseconds have passed.
/// NULL on a timeout ("a 'nil' value is returned"). The topic is the pointer
/// `zerodds_dp_create_topic` returned; one `zerodds_dp_delete_topic` deletes
/// it, whatever the number of times it was found (the C topic is a pointer,
/// not a counted proxy).
///
/// # Safety
/// `p`, `name` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_find_topic_w_timeout(
    p: *mut ZeroDdsDomainParticipant,
    name: *const c_char,
    timeout_ms: u64,
) -> *mut ZeroDdsTopic {
    if p.is_null() || name.is_null() {
        return ptr::null_mut();
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        // SAFETY: `p` and `name` NULL-checked above; the caller's pledge.
        let found = unsafe { zerodds_dp_find_topic(p, name) };
        if !found.is_null() || std::time::Instant::now() >= deadline {
            return found;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

// ---------------------------------------------------------------------------
// Publisher
// ---------------------------------------------------------------------------

/// Creates a publisher.
///
/// # Safety
/// `p` valid; `qos` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_create_publisher(
    p: *mut ZeroDdsDomainParticipant,
    qos: *const ZeroDdsPublisherQos,
) -> *mut ZeroDdsPublisher {
    if p.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; qos NULL-tolerant.
    unsafe {
        let pp = &*p;
        let qos: PublisherQos = if qos.is_null() {
            pp.default_publisher_qos
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default()
        } else {
            pub_qos_from_c(qos)
        };
        let pub_ = Box::new(ZeroDdsPublisher {
            participant: p,
            qos: Mutex::new(qos),
            default_dw_qos: Mutex::new(DataWriterQos::default()),
            datawriters: Mutex::new(Vec::new()),
            suspended: Mutex::new(0),
            coherent_depth: Mutex::new(0),
            entity_id: pp.rt.as_ref().map(|rt| {
                rt.new_group_entity_id(zerodds_rtps::wire_types::EntityKind::UserWriterGroup)
            }),
            group: Mutex::new(None),
            partition_out: Mutex::new(Default::default()),
            lifecycle: crate::entity_ffi::Lifecycle::new(
                crate::entity_ffi::participant_autoenables(pp),
            ),
        });
        let pp_ptr = Box::into_raw(pub_);
        if let Ok(mut list) = pp.publishers.lock() {
            list.push(pp_ptr);
        }
        pp_ptr
    }
}

/// Deletes a publisher.
///
/// # Safety
/// Both handles valid; the publisher must contain no more DataWriters.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_publisher(
    p: *mut ZeroDdsDomainParticipant,
    pubh: *mut ZeroDdsPublisher,
) -> c_int {
    if p.is_null() || pubh.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p+pubh NULL-checked above; pubh from create_publisher.
    unsafe {
        let pp = &*p;
        let pb = &*pubh;
        if pb.participant != p {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        let has_dws = pb
            .datawriters
            .lock()
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if has_dws {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        if let Ok(mut list) = pp.publishers.lock() {
            let n = list.len();
            list.retain(|x| *x != pubh);
            if list.len() == n {
                return ZeroDdsStatus::BadHandle as c_int;
            }
        }
        let _ = Box::from_raw(pubh);
    }
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// Subscriber
// ---------------------------------------------------------------------------

/// Creates a subscriber.
///
/// # Safety
/// `p` valid; `qos` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_create_subscriber(
    p: *mut ZeroDdsDomainParticipant,
    qos: *const ZeroDdsSubscriberQos,
) -> *mut ZeroDdsSubscriber {
    if p.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; qos NULL-tolerant.
    unsafe {
        let pp = &*p;
        let qos: SubscriberQos = if qos.is_null() {
            pp.default_subscriber_qos
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default()
        } else {
            sub_qos_from_c(qos)
        };
        let group = alloc::sync::Arc::new(crate::entities::SubscriberGroup::default());
        // HEARTBEATs and GAPs with group information, and the tick, let the
        // group order commit (DDSI-RTPS 2.5 §8.7.5).
        let group_hook = pp.rt.as_ref().map(|rt| {
            let weak_group = alloc::sync::Arc::downgrade(&group);
            let weak_rt = alloc::sync::Arc::downgrade(rt);
            let hook: zerodds_dcps::runtime::StatusHook = alloc::sync::Arc::new(move || {
                if let (Some(group), Some(rt)) = (weak_group.upgrade(), weak_rt.upgrade()) {
                    group.release(&rt);
                }
            });
            rt.add_group_hook(&hook);
            hook
        });
        let sub_ = Box::new(ZeroDdsSubscriber {
            participant: p,
            qos: Mutex::new(qos),
            default_dr_qos: Mutex::new(DataReaderQos::default()),
            datareaders: Mutex::new(Vec::new()),
            partition_out: Mutex::new(Default::default()),
            entity_id: pp.rt.as_ref().map(|rt| {
                rt.new_group_entity_id(zerodds_rtps::wire_types::EntityKind::UserReaderGroup)
            }),
            group,
            group_hook: Mutex::new(group_hook),
            access: Default::default(),
            lifecycle: crate::entity_ffi::Lifecycle::new(
                crate::entity_ffi::participant_autoenables(pp),
            ),
        });
        let sptr = Box::into_raw(sub_);
        crate::entities::note_status_entity(sptr, crate::entities::StatusEntityKind::Subscriber);
        if let Ok(mut list) = pp.subscribers.lock() {
            list.push(sptr);
        }
        sptr
    }
}

/// Deletes a subscriber.
///
/// # Safety
/// Both handles valid; the subscriber must hold no more DataReaders.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_subscriber(
    p: *mut ZeroDdsDomainParticipant,
    sub: *mut ZeroDdsSubscriber,
) -> c_int {
    if p.is_null() || sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p+sub NULL-checked above; sub from create_subscriber.
    unsafe {
        let pp = &*p;
        let sb = &*sub;
        if sb.participant != p {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        let has_drs = sb
            .datareaders
            .lock()
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if has_drs {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        if let Ok(mut list) = pp.subscribers.lock() {
            let n = list.len();
            list.retain(|x| *x != sub);
            if list.len() == n {
                return ZeroDdsStatus::BadHandle as c_int;
            }
        }
        let _ = Box::from_raw(sub);
    }
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// ContentFilteredTopic
// ---------------------------------------------------------------------------

/// Creates a ContentFilteredTopic.
///
/// # Safety
/// `p`, `name`, `related`, `filter_expression` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_create_contentfilteredtopic(
    p: *mut ZeroDdsDomainParticipant,
    name: *const c_char,
    related: *mut ZeroDdsTopic,
    filter_expression: *const c_char,
    parameters: *const *const c_char,
    param_count: usize,
) -> *mut ZeroDdsContentFilteredTopic {
    if p.is_null() || related.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — p+related NULL-checked above; name/filter/parameters
    // per caller pledge; parameters[0..param_count] must be valid if parameters != NULL.
    unsafe {
        let name_s = match cstr_to_str(name) {
            Ok(s) if !s.is_empty() => s.to_string(),
            _ => return ptr::null_mut(),
        };
        let expr_s = match cstr_to_str(filter_expression) {
            Ok(s) => s.to_string(),
            Err(_) => return ptr::null_mut(),
        };
        let mut params: Vec<String> = Vec::with_capacity(param_count);
        if !parameters.is_null() && param_count > 0 {
            let slc = slice::from_raw_parts(parameters, param_count);
            for &cp in slc {
                match cstr_to_str(cp) {
                    Ok(s) => params.push(s.to_string()),
                    Err(_) => return ptr::null_mut(),
                }
            }
        }
        let values = std::sync::Arc::new(std::sync::RwLock::new(
            params
                .iter()
                .map(|p| zerodds_sql_filter::Value::from_parameter(p))
                .collect(),
        ));
        let cft = Box::new(ZeroDdsContentFilteredTopic {
            participant: p,
            related_topic: related,
            name: name_s,
            filter_expression: expr_s,
            parameters: Mutex::new(params),
            schema: Mutex::new(Vec::new()),
            extensibility: Mutex::new(crate::entities::CftExtensibility::default()),
            typed: Mutex::new(None),
            values,
            readers: Mutex::new(Vec::new()),
            live_readers: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let raw = Box::into_raw(cft);
        // The participant keeps the list `delete_topic` counts: a
        // ContentFilteredTopic that exists holds its related topic
        // (DDS 1.4 §2.2.2.2.1.6).
        if let Ok(mut list) = (*p).content_filtered_topics.lock() {
            list.push(raw);
        }
        raw
    }
}

/// Replaces the filter parameters of a ContentFilteredTopic (DDS 1.4
/// §2.2.2.3.3 `set_expression_parameters`). Readers already created on it
/// filter with the new values from their next take and announce them in
/// their SEDP subscription. `BadParameter` when the expression names a
/// parameter beyond `count`.
///
/// # Safety
/// `cft` valid; `parameters[0..count]` NUL-terminated strings or `count == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_cft_set_filter_parameters(
    cft: *mut ZeroDdsContentFilteredTopic,
    parameters: *const *const c_char,
    count: usize,
) -> c_int {
    if cft.is_null() || (count > 0 && parameters.is_null()) {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: cft NULL-checked; `parameters[0..count]` readable per the caller.
    unsafe {
        let cft = &*cft;
        let mut raw = Vec::with_capacity(count);
        let given: &[*const c_char] = if count == 0 {
            &[]
        } else {
            slice::from_raw_parts(parameters, count)
        };
        for &p in given {
            match cstr_to_str(p) {
                Ok(s) => raw.push(s.to_string()),
                Err(_) => return ZeroDdsStatus::BadParameter as c_int,
            }
        }
        let Ok(expr) = zerodds_sql_filter::parse(&cft.filter_expression) else {
            return ZeroDdsStatus::BadParameter as c_int;
        };
        if expr
            .collect_param_indices()
            .iter()
            .any(|&i| i as usize >= raw.len())
        {
            return ZeroDdsStatus::BadParameter as c_int;
        }
        let values: Vec<zerodds_sql_filter::Value> = raw
            .iter()
            .map(|p| zerodds_sql_filter::Value::from_parameter(p))
            .collect();
        match (cft.parameters.lock(), cft.values.write()) {
            (Ok(mut p), Ok(mut v)) => {
                *p = raw.clone();
                *v = values;
            }
            _ => return ZeroDdsStatus::Error as c_int,
        }
        if cft.related_topic.is_null() {
            return ZeroDdsStatus::Ok as c_int;
        }
        let related = (*cft.related_topic).name.clone();
        let property = zerodds_rtps::subscription_data::ContentFilterProperty {
            content_filtered_topic_name: cft.name.clone(),
            related_topic_name: related,
            filter_class_name: zerodds_rtps::subscription_data::filter_class::DDSSQL.into(),
            filter_expression: cft.filter_expression.clone(),
            expression_parameters: raw,
        };
        // A reader deleted since has no announcement left; it is dropped.
        if let Ok(mut readers) = cft.readers.lock() {
            readers.retain(|(rt, eid)| {
                rt.update_user_reader_content_filter(*eid, Some(property.clone()))
                    .is_ok()
                    // A reader still disabled announces the parameters it
                    // has when it is enabled.
                    || crate::entity_ffi::patch_pending_reader_filter(
                        cft.participant,
                        *eid,
                        &property,
                    )
            });
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// The number of filter parameters of a ContentFilteredTopic; 0 for NULL.
///
/// # Safety
/// `cft` valid or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_cft_get_filter_parameter_count(
    cft: *mut ZeroDdsContentFilteredTopic,
) -> usize {
    if cft.is_null() {
        return 0;
    }
    // SAFETY: NULL-checked.
    unsafe { &*cft }.parameters.lock().map_or(0, |p| p.len())
}

/// Filter parameter `index` of a ContentFilteredTopic as a new string the
/// caller frees with `zerodds_string_free`; NULL when out of range.
///
/// # Safety
/// `cft` valid or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_cft_get_filter_parameter(
    cft: *mut ZeroDdsContentFilteredTopic,
    index: usize,
) -> *mut c_char {
    if cft.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: NULL-checked.
    let param = unsafe { &*cft }
        .parameters
        .lock()
        .ok()
        .and_then(|p| p.get(index).cloned());
    match param.and_then(|p| std::ffi::CString::new(p).ok()) {
        Some(c) => c.into_raw(),
        None => ptr::null_mut(),
    }
}

/// Declares the related topic's type through its `CompleteTypeObject`
/// (XTypes 1.3 §7.3.4, the `to_bytes_le` form `idlc cpp` emits as
/// `topic_type_support<T>::type_object()`), so a DataReader on this
/// ContentFilteredTopic decodes each sample and reads the filter's fields by
/// name, nested members as `a.b`. `is_keyed` and `extensibility`
/// (`0 = @final`, `1 = @appendable`, `2 = @mutable`) must agree with the
/// object. Set it before `zerodds_sub_create_datareader_with_cft`.
///
/// Returns `BadParameter` for a NULL argument, a malformed object, one that
/// needs dependency objects this call does not carry, or flags that
/// disagree with it.
///
/// # Safety
/// `cft` valid; `type_object` points to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_cft_set_type_object(
    cft: *mut ZeroDdsContentFilteredTopic,
    type_object: *const u8,
    len: usize,
    is_keyed: c_int,
    extensibility: u32,
) -> c_int {
    if cft.is_null() || type_object.is_null() || len == 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let Ok(extensibility) = u8::try_from(extensibility) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    // SAFETY: cft and type_object NULL-checked; `type_object[0..len]` readable
    // and `cft.related_topic` valid per the caller's pledge.
    unsafe {
        let cft = &*cft;
        if cft.related_topic.is_null() {
            return ZeroDdsStatus::BadParameter as c_int;
        }
        let type_name = (*cft.related_topic).type_name.clone();
        let bytes = slice::from_raw_parts(type_object, len);
        match zerodds_dcps::serialized_type::SerializedTypeSupport::from_type_object(
            &type_name,
            bytes,
            is_keyed != 0,
            extensibility,
        ) {
            Ok(support) => match cft.typed.lock() {
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

/// [`zerodds_cft_set_type_object`] for a type that refers to other types:
/// `dependencies[0..count]` (each `dependency_lens[i]` bytes) are the
/// complete TypeObjects the root refers to, transitively, as `idlc cpp`
/// emits them (`topic_type_support<T>::type_object_dependency(i)`).
/// `BadParameter` when one is missing or malformed.
///
/// # Safety
/// `cft` valid; `type_object` readable for `len` bytes; `dependencies` and
/// `dependency_lens` readable for `count` entries, each dependency for its
/// length; or `count == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_cft_set_type_objects(
    cft: *mut ZeroDdsContentFilteredTopic,
    type_object: *const u8,
    len: usize,
    dependencies: *const *const u8,
    dependency_lens: *const usize,
    count: usize,
    is_keyed: c_int,
    extensibility: u32,
) -> c_int {
    if cft.is_null()
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
        let cft = &*cft;
        if cft.related_topic.is_null() {
            return ZeroDdsStatus::BadParameter as c_int;
        }
        let mut deps: Vec<Vec<u8>> = Vec::with_capacity(count);
        for i in 0..count {
            let p = *dependencies.add(i);
            let n = *dependency_lens.add(i);
            if p.is_null() || n == 0 {
                return ZeroDdsStatus::BadParameter as c_int;
            }
            deps.push(slice::from_raw_parts(p, n).to_vec());
        }
        let type_name = (*cft.related_topic).type_name.clone();
        let root = slice::from_raw_parts(type_object, len);
        match zerodds_dcps::serialized_type::SerializedTypeSupport::from_type_objects(
            &type_name,
            root,
            &deps,
            is_keyed != 0,
            extensibility,
        ) {
            Ok(support) => match cft.typed.lock() {
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

/// Sets the positional CDR field schema on a ContentFilteredTopic so the
/// untyped C-FFI filter actually evaluates the payload (Spec §2.2.2.3.3).
///
/// `names[i]` is the field name referenced by the filter expression;
/// `kinds[i]` is its CDR encoding (0=bool, 1=int32, 2=int64, 3=float32,
/// 4=float64, 5=string). The fields MUST be listed in their on-wire
/// declaration order, starting from the first member of the payload.
///
/// This variant assumes a `@final` payload (no leading DHEADER, offset 0). For
/// an `@appendable`/`@mutable` payload use [`zerodds_cft_set_schema_ex`].
///
/// Note: filter literals are passed as-is — the SQL `WHERE` expression carries
/// its own typed literals (e.g. `seq > 4`, `name = 'foo'`); the schema only
/// declares how to decode the payload columns, it does not coerce the literals.
///
/// # Safety
/// `cft` valid; `names[0..count]` valid NUL-terminated C strings;
/// `kinds[0..count]` readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_cft_set_schema(
    cft: *mut ZeroDdsContentFilteredTopic,
    names: *const *const c_char,
    kinds: *const u32,
    count: usize,
) -> c_int {
    // SAFETY: forwards the same pledges to the `_ex` variant; `extensibility=0`
    // (Final) preserves the historical offset-0 behaviour.
    unsafe { zerodds_cft_set_schema_ex(cft, names, kinds, count, 0) }
}

/// Like [`zerodds_cft_set_schema`] but additionally declares the payload's type
/// `extensibility` (XTypes 1.3 §7.3.1.2.1): `0 = @final`, `1 = @appendable`,
/// `2 = @mutable`. For `@appendable`/`@mutable` the XCDR2 stream is prefixed
/// with a 4-byte DHEADER (§7.4.3.4.2) before the first member, so the
/// positional decoder skips 4 bytes; for `@final` the first member is at
/// offset 0. Any other value is rejected.
///
/// Filter literals are passed as-is (see [`zerodds_cft_set_schema`]).
///
/// # Safety
/// `cft` valid; `names[0..count]` valid NUL-terminated C strings;
/// `kinds[0..count]` readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_cft_set_schema_ex(
    cft: *mut ZeroDdsContentFilteredTopic,
    names: *const *const c_char,
    kinds: *const u32,
    count: usize,
    extensibility: u32,
) -> c_int {
    use crate::entities::{CftExtensibility, CftField, CftFieldKind};
    if cft.is_null() || (count > 0 && (names.is_null() || kinds.is_null())) {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let ext = match extensibility {
        0 => CftExtensibility::Final,
        // @mutable also carries a leading DHEADER in XCDR2 (§7.4.3.4.2); for
        // the positional decoder it behaves like @appendable.
        1 | 2 => CftExtensibility::Appendable,
        _ => return ZeroDdsStatus::BadParameter as c_int,
    };
    // SAFETY: see fn # Safety doc — cft+names+kinds NULL-checked above;
    // names[0..count]/kinds[0..count] readable per caller pledge.
    unsafe {
        let mut fields: Vec<CftField> = Vec::with_capacity(count);
        if count > 0 {
            let names_slc = slice::from_raw_parts(names, count);
            let kinds_slc = slice::from_raw_parts(kinds, count);
            for (cp, &k) in names_slc.iter().zip(kinds_slc.iter()) {
                let name = match cstr_to_str(*cp) {
                    Ok(s) if !s.is_empty() => s.to_string(),
                    _ => return ZeroDdsStatus::BadParameter as c_int,
                };
                let kind = match k {
                    0 => CftFieldKind::Bool,
                    1 => CftFieldKind::Int32,
                    2 => CftFieldKind::Int64,
                    3 => CftFieldKind::Float32,
                    4 => CftFieldKind::Float64,
                    5 => CftFieldKind::StringField,
                    _ => return ZeroDdsStatus::BadParameter as c_int,
                };
                fields.push(CftField { name, kind });
            }
        }
        if let Ok(mut g) = (*cft).schema.lock() {
            *g = fields;
        }
        if let Ok(mut g) = (*cft).extensibility.lock() {
            *g = ext;
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Deletes a ContentFilteredTopic.
///
/// # Safety
/// Both handles valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_contentfilteredtopic(
    p: *mut ZeroDdsDomainParticipant,
    cft: *mut ZeroDdsContentFilteredTopic,
) -> c_int {
    if p.is_null() || cft.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p+cft NULL-checked above; cft from create_contentfilteredtopic.
    unsafe {
        let pp = &*p;
        if (*cft).participant != p {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        // DDS 1.4 §2.2.2.2.1.8: "If the delete_contentfilteredtopic
        // operation is called on a ContentFilteredTopic with existing
        // DataReader objects attached to it, it will return
        // PRECONDITION_NOT_MET."
        if (*cft)
            .live_readers
            .load(std::sync::atomic::Ordering::Acquire)
            > 0
        {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        // Out of the participant's list first: `delete_topic` counts the
        // ContentFilteredTopics that hold a topic (DDS 1.4 §2.2.2.2.1.6),
        // and a handle freed while still counted would refuse that topic for
        // the rest of the process.
        let mut known = true;
        if let Ok(mut list) = pp.content_filtered_topics.lock() {
            let before = list.len();
            list.retain(|x| *x != cft);
            known = list.len() != before;
        }
        if !known {
            return ZeroDdsStatus::BadHandle as c_int;
        }
        let _ = Box::from_raw(cft);
    }
    ZeroDdsStatus::Ok as c_int
}

// ---------------------------------------------------------------------------
// Ignore-API (Spec §2.2.2.2.1.6 .. .9)
// ---------------------------------------------------------------------------

/// Ignore Participant by InstanceHandle.
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_ignore_participant(
    p: *mut ZeroDdsDomainParticipant,
    handle: u64,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    use zerodds_dcps::instance_handle::InstanceHandle;
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    let rc = unsafe { (*p).dp.ignore_participant(InstanceHandle::from_raw(handle)) };
    match rc {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(zerodds_dcps::DdsError::NotEnabled) => ZeroDdsStatus::NotEnabled as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Ignore Topic by InstanceHandle.
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_ignore_topic(
    p: *mut ZeroDdsDomainParticipant,
    handle: u64,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    use zerodds_dcps::instance_handle::InstanceHandle;
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    let rc = unsafe { (*p).dp.ignore_topic(InstanceHandle::from_raw(handle)) };
    match rc {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(zerodds_dcps::DdsError::NotEnabled) => ZeroDdsStatus::NotEnabled as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Ignore Publication by InstanceHandle.
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_ignore_publication(
    p: *mut ZeroDdsDomainParticipant,
    handle: u64,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    use zerodds_dcps::instance_handle::InstanceHandle;
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    let rc = unsafe { (*p).dp.ignore_publication(InstanceHandle::from_raw(handle)) };
    match rc {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(zerodds_dcps::DdsError::NotEnabled) => ZeroDdsStatus::NotEnabled as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Ignore Subscription by InstanceHandle.
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_ignore_subscription(
    p: *mut ZeroDdsDomainParticipant,
    handle: u64,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    use zerodds_dcps::instance_handle::InstanceHandle;
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    let rc = unsafe {
        (*p).dp
            .ignore_subscription(InstanceHandle::from_raw(handle))
    };
    match rc {
        Ok(()) => ZeroDdsStatus::Ok as c_int,
        Err(zerodds_dcps::DdsError::NotEnabled) => ZeroDdsStatus::NotEnabled as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

/// Domain ID of the participant.
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_domain_id(p: *mut ZeroDdsDomainParticipant) -> u32 {
    if p.is_null() {
        return u32::MAX;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    unsafe { (*p).domain_id }
}

/// Asserts liveliness for all MANUAL_BY_PARTICIPANT writers held by the
/// participant.
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_assert_liveliness(p: *mut ZeroDdsDomainParticipant) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    if let Some(rt) = unsafe { (*p).rt.as_ref() } {
        rt.assert_liveliness();
    }
    ZeroDdsStatus::Ok as c_int
}

/// Writes the participant's current time into `*out`: "the time that the
/// service uses to time-stamp data-writes" (DDS 1.4 §2.2.2.2.1.32), the
/// runtime's `time_source`. It read the system clock.
///
/// # Safety
/// `p` and `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_current_time(
    p: *mut ZeroDdsDomainParticipant,
    out: *mut ZeroDdsTime,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: p NULL-checked above.
    let now = match unsafe { (*p).dp.get_current_time() } {
        Ok(now) => now,
        Err(zerodds_dcps::DdsError::NotEnabled) => return ZeroDdsStatus::NotEnabled as c_int,
        Err(_) => return ZeroDdsStatus::Error as c_int,
    };
    // SAFETY: out NULL-checked above.
    unsafe {
        *out = ZeroDdsTime {
            sec: now.sec,
            nanosec: now.nanosec,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// Deletes all topics, publishers, subscribers held by the participant
/// recursively (Spec §2.2.2.2.1.10).
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_contained_entities(
    p: *mut ZeroDdsDomainParticipant,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; all lists hold only
    // pointers from Box::into_raw of the respective create_* fns.
    unsafe {
        let pp = &*p;

        // Order: first Pub/Sub (incl. their DW/DR), then topics.
        let pubs: Vec<*mut ZeroDdsPublisher> = pp
            .publishers
            .lock()
            .map(|mut g| core::mem::take(&mut *g))
            .unwrap_or_default();
        for pub_ptr in pubs {
            if pub_ptr.is_null() {
                continue;
            }
            // delete_contained_entities on the publisher: inline here
            // (the publisher FFI implements this in publisher_ffi.rs, but
            // we tear down locally).
            let pb = &*pub_ptr;
            if let Ok(mut dws) = pb.datawriters.lock() {
                for dw in dws.drain(..) {
                    if !dw.is_null() {
                        let _ = Box::from_raw(dw);
                    }
                }
            }
            let _ = Box::from_raw(pub_ptr);
        }

        let subs: Vec<*mut ZeroDdsSubscriber> = pp
            .subscribers
            .lock()
            .map(|mut g| core::mem::take(&mut *g))
            .unwrap_or_default();
        for sub_ptr in subs {
            if sub_ptr.is_null() {
                continue;
            }
            let sb = &*sub_ptr;
            if let Ok(mut drs) = sb.datareaders.lock() {
                for dr in drs.drain(..) {
                    if !dr.is_null() {
                        let _ = Box::from_raw(dr);
                    }
                }
            }
            let _ = Box::from_raw(sub_ptr);
        }

        let topics: Vec<*mut ZeroDdsTopic> = pp
            .topics
            .lock()
            .map(|mut g| core::mem::take(&mut *g))
            .unwrap_or_default();
        for t in topics {
            if !t.is_null() {
                let _ = Box::from_raw(t);
            }
        }

        // The ContentFilteredTopics are freed last: each holds the
        // `related_topic` pointer it was created with, and a filtered topic
        // outlives nothing that points at it here.
        let cfts: Vec<*mut ZeroDdsContentFilteredTopic> = pp
            .content_filtered_topics
            .lock()
            .map(|mut g| core::mem::take(&mut *g))
            .unwrap_or_default();
        for cft in cfts {
            if !cft.is_null() {
                let _ = Box::from_raw(cft);
            }
        }
    }
    ZeroDdsStatus::Ok as c_int
}

/// Checks whether an InstanceHandle belongs to an entity in this participant
/// (Spec §2.2.2.2.1.11).
///
/// # Safety
/// `p` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_contains_entity(
    p: *mut ZeroDdsDomainParticipant,
    handle: u64,
) -> c_int {
    if p.is_null() {
        return 0;
    }
    use zerodds_dcps::instance_handle::InstanceHandle;
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    let contains = unsafe {
        (*p).dp.contains_entity(InstanceHandle::from_raw(handle))
            || crate::entity_ffi::contains_entity(p, InstanceHandle::from_raw(handle))
    };
    if contains { 1 } else { 0 }
}

// ---------------------------------------------------------------------------
// Discovery-Listing
// ---------------------------------------------------------------------------

/// Returns the InstanceHandles of all discovered remote participants.
///
/// The first `min(len, cap)` handles are written into `out_handles`,
/// `*out_count` is set to the **total** number of discovered participants, and
/// the return value is `OutOfResources` when `len > cap` (so a caller retries
/// with a buffer of `*out_count`), `Ok` otherwise. A NULL `out_handles` with
/// `cap == 0` asks for the length alone (`docs/specs/zerodds-c-api-1.0.md`
/// §2.8).
///
/// # Safety
/// `p`, `out_count` valid; `out_handles[0..cap]` writeable, or `out_handles`
/// NULL exactly when `cap == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_discovered_participants(
    p: *mut ZeroDdsDomainParticipant,
    out_handles: *mut u64,
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
            .get_discovered_participants()
            .iter()
            .map(|h| h.as_raw())
            .collect();
        copy_list_out(out_handles, cap, &handles, out_count)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::factory_ffi::{
        zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
    };
    use crate::publisher_ffi::{zerodds_pub_create_datawriter, zerodds_pub_delete_datawriter};
    use crate::subscriber_ffi::{zerodds_sub_create_datareader, zerodds_sub_delete_datareader};

    fn mk_participant(domain: u32) -> *mut ZeroDdsDomainParticipant {
        let f = zerodds_dpf_get_instance();
        // SAFETY: f from dpf_get_instance, statically valid.
        unsafe { zerodds_dpf_create_participant(f, domain, ptr::null()) }
    }

    fn drop_participant(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: p from mk_participant; f static.
        unsafe {
            zerodds_dp_delete_contained_entities(p);
            zerodds_dpf_delete_participant(f, p);
        }
    }

    #[test]
    fn create_topic_then_find_then_delete() {
        let p = mk_participant(11);
        assert!(!p.is_null());
        let n = c"MyTopic";
        let tn = c"MyType";
        // SAFETY: p from mk; n+tn static.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            assert!(!t.is_null());
            let f = zerodds_dp_find_topic(p, n.as_ptr());
            assert_eq!(f, t);
            let rc = zerodds_dp_delete_topic(p, t);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert!(zerodds_dp_find_topic(p, n.as_ptr()).is_null());
        }
        drop_participant(p);
    }

    /// DDS 1.4 §2.2.2.2.1.6: "The deletion of a Topic is not allowed if
    /// there are any existing DataReader, DataWriter, ContentFilteredTopic,
    /// or MultiTopic objects that are using the Topic… it will return
    /// PRECONDITION_NOT_MET."
    #[test]
    fn delete_topic_is_refused_while_a_datawriter_uses_it() {
        let p = mk_participant(81);
        let n = c"Held";
        let tn = c"TT";
        // SAFETY: p from mk; n+tn static.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
            assert!(!dw.is_null());
            assert_eq!(
                zerodds_dp_delete_topic(p, t),
                ZeroDdsStatus::PreconditionNotMet as c_int
            );
            // Refused means "no effect": the topic is still there.
            assert_eq!(zerodds_dp_find_topic(p, n.as_ptr()), t);
            assert_eq!(
                zerodds_pub_delete_datawriter(pubh, dw),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(zerodds_dp_delete_topic(p, t), ZeroDdsStatus::Ok as c_int);
        }
        drop_participant(p);
    }

    /// The DataReader clause of the same §2.2.2.2.1.6 sentence.
    #[test]
    fn delete_topic_is_refused_while_a_datareader_uses_it() {
        let p = mk_participant(82);
        let n = c"HeldByReader";
        let tn = c"TT";
        // SAFETY: p from mk; n+tn static.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
            assert!(!dr.is_null());
            assert_eq!(
                zerodds_dp_delete_topic(p, t),
                ZeroDdsStatus::PreconditionNotMet as c_int
            );
            assert_eq!(
                zerodds_sub_delete_datareader(sub, dr),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(zerodds_dp_delete_topic(p, t), ZeroDdsStatus::Ok as c_int);
        }
        drop_participant(p);
    }

    /// The ContentFilteredTopic clause of the same sentence.
    #[test]
    fn delete_topic_is_refused_while_a_content_filtered_topic_uses_it() {
        let p = mk_participant(83);
        let n = c"HeldByCft";
        let tn = c"TT";
        let cn = c"Cft";
        let expr = c"x > %0";
        let one: [*const c_char; 1] = [c"0".as_ptr()];
        // SAFETY: p from mk; the C strings are static/stack-local.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let cft = zerodds_dp_create_contentfilteredtopic(
                p,
                cn.as_ptr(),
                t,
                expr.as_ptr(),
                one.as_ptr(),
                1,
            );
            assert!(!cft.is_null());
            assert_eq!(
                zerodds_dp_delete_topic(p, t),
                ZeroDdsStatus::PreconditionNotMet as c_int
            );
            assert_eq!(
                zerodds_dp_delete_contentfilteredtopic(p, cft),
                ZeroDdsStatus::Ok as c_int
            );
            assert_eq!(zerodds_dp_delete_topic(p, t), ZeroDdsStatus::Ok as c_int);
        }
        drop_participant(p);
    }

    /// DDS-XTypes 1.3 §7.6.4.1: a second type under the name of a topic is
    /// another topic when its QoS equals the first one's, and a conflict
    /// (NULL) when it does not.
    #[test]
    fn topic_name_collision_with_different_type_follows_the_qos() {
        let p = mk_participant(12);
        let n = c"X";
        let t1 = c"TypeA";
        let t2 = c"TypeB";
        let t3 = c"TypeC";
        // SAFETY: p from mk; n+t1+t2+t3 static; `other` lives for the test.
        unsafe {
            let a = zerodds_dp_create_topic(p, n.as_ptr(), t1.as_ptr(), ptr::null());
            assert!(!a.is_null());
            let b = zerodds_dp_create_topic(p, n.as_ptr(), t2.as_ptr(), ptr::null());
            assert!(
                !b.is_null(),
                "another type with the same QoS is another topic"
            );
            let other: crate::qos_ffi::ZeroDdsTopicQos = core::mem::zeroed();
            let c = zerodds_dp_create_topic(p, n.as_ptr(), t3.as_ptr(), &other);
            assert!(c.is_null(), "another type with another QoS is a conflict");
        }
        drop_participant(p);
    }

    #[test]
    fn create_delete_publisher_clean() {
        let p = mk_participant(13);
        // SAFETY: p from mk.
        unsafe {
            let pubh = zerodds_dp_create_publisher(p, ptr::null());
            assert!(!pubh.is_null());
            let rc = zerodds_dp_delete_publisher(p, pubh);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        }
        drop_participant(p);
    }

    #[test]
    fn create_delete_subscriber_clean() {
        let p = mk_participant(14);
        // SAFETY: p from mk.
        unsafe {
            let sub = zerodds_dp_create_subscriber(p, ptr::null());
            assert!(!sub.is_null());
            let rc = zerodds_dp_delete_subscriber(p, sub);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        }
        drop_participant(p);
    }

    #[test]
    fn delete_contained_entities_drops_all() {
        let p = mk_participant(15);
        let n = c"T";
        let tn = c"TT";
        let f = zerodds_dpf_get_instance();
        // SAFETY: p from mk; n+tn static; f static.
        unsafe {
            let _t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let _pubh = zerodds_dp_create_publisher(p, ptr::null());
            let _sub = zerodds_dp_create_subscriber(p, ptr::null());
            let rc = zerodds_dp_delete_contained_entities(p);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            // Now delete_participant may proceed.
            let rc2 = zerodds_dpf_delete_participant(f, p);
            assert_eq!(rc2, ZeroDdsStatus::Ok as c_int);
        }
    }

    #[test]
    fn domain_id_roundtrip() {
        let p = mk_participant(99);
        // SAFETY: p from mk.
        assert_eq!(unsafe { zerodds_dp_get_domain_id(p) }, 99);
        drop_participant(p);
    }

    #[test]
    fn get_current_time_returns_nonzero() {
        let p = mk_participant(98);
        let mut t = ZeroDdsTime { sec: 0, nanosec: 0 };
        // SAFETY: p from mk; t is a live stack slot.
        let rc = unsafe { zerodds_dp_get_current_time(p, &mut t) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert!(t.sec > 0, "wall-clock seconds should be positive");
        assert!(t.nanosec < 1_000_000_000, "nanosec in range");
        // NULL out → BadHandle.
        assert_eq!(
            unsafe { zerodds_dp_get_current_time(p, ptr::null_mut()) },
            ZeroDdsStatus::BadHandle as c_int
        );
        drop_participant(p);
    }

    #[test]
    fn cft_create_delete() {
        let p = mk_participant(16);
        let n = c"T";
        let tn = c"TT";
        let cn = c"CFT";
        let expr = c"x > %0";
        let p1 = c"42";
        let params: [*const c_char; 1] = [p1.as_ptr()];
        // SAFETY: p from mk; all CStr+param array static/stack-local.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            assert!(!t.is_null());
            let cft = zerodds_dp_create_contentfilteredtopic(
                p,
                cn.as_ptr(),
                t,
                expr.as_ptr(),
                params.as_ptr(),
                1,
            );
            assert!(!cft.is_null());
            let rc = zerodds_dp_delete_contentfilteredtopic(p, cft);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            let _ = zerodds_dp_delete_topic(p, t);
        }
        drop_participant(p);
    }

    #[test]
    fn ignore_participant_returns_status() {
        let p = mk_participant(17);
        // SAFETY: p from mk.
        let rc = unsafe { zerodds_dp_ignore_participant(p, 12345) };
        // Valid handle path: rt may return Error if handle unknown — both Ok.
        assert!(rc == ZeroDdsStatus::Ok as c_int || rc == ZeroDdsStatus::Error as c_int);
        drop_participant(p);
    }

    /// `cap == 0` with a NULL buffer asks for the total length; a NULL buffer
    /// with `cap > 0` is refused (`docs/specs/zerodds-c-api-1.0.md` §2.8).
    #[test]
    fn discovered_participants_length_query_and_null_contract() {
        let p = mk_participant(18);
        let mut count = 0usize;
        // SAFETY: p from mk_participant; count stack-local.
        unsafe {
            let rc = zerodds_dp_get_discovered_participants(p, ptr::null_mut(), &mut count, 0);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            assert_eq!(count, 0);
            let rc = zerodds_dp_get_discovered_participants(p, ptr::null_mut(), &mut count, 8);
            assert_eq!(rc, ZeroDdsStatus::BadParameter as c_int);
        }
        drop_participant(p);
    }
}
