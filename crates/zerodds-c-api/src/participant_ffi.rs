// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DomainParticipant C-FFI (Spec §2.2.2.2.1 + DDS-PSM-Cxx §7.5.2).
//!
//! Implementiert die C-Surface der DomainParticipant-Operationen aus
//! `docs/specs/zerodds-c-api-1.0.md` §2.2. QoS-Pointer akzeptieren in
//! der RC1-Surface NULL (Default) oder werden in `qos.rs` definitiv
//! mit `#[repr(C)]`-Layouts hinterlegt — die Funktionssignaturen hier
//! ankern bereits den finalen ABI-Vertrag.

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
use crate::qos_ffi::{
    ZeroDdsPublisherQos, ZeroDdsSubscriberQos, ZeroDdsTopicQos, pub_qos_from_c, sub_qos_from_c,
    topic_qos_from_c,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Sicheres Cast `*const c_char` → `&str`. Liefert `Err` bei NULL oder
/// wenn nicht UTF-8.
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe fn cstr_to_str<'a>(p: *const c_char) -> Result<&'a str, ()> {
    if p.is_null() {
        return Err(());
    }
    // SAFETY: NULL-Check oben + Caller-Kontrakt fuer C-string termination.
    let cs = unsafe { CStr::from_ptr(p) };
    cs.to_str().map_err(|_| ())
}

// ---------------------------------------------------------------------------
// Topic
// ---------------------------------------------------------------------------

/// Erzeugt ein Topic. Liefert NULL bei NULL-Argumenten oder Topic-Name-
/// Konflikt mit anderem Type.
///
/// # Safety
/// `p`, `name`, `type_name` muessen valide sein. `qos` darf NULL sein
/// (Default aus `default_topic_qos`).
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
    // per Caller-Pledge; topics-Liste haelt nur Pointer aus Box::into_raw.
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

        // QoS-Pfad: Caller-supplied wenn non-NULL, sonst Participant-Default.
        let qos: TopicQos = if qos.is_null() {
            pp.default_topic_qos
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default()
        } else {
            topic_qos_from_c(qos)
        };

        // Konflikt-Check: existiert ein Topic gleichen Namens mit anderem Type?
        if let Ok(list) = pp.topics.lock() {
            for &existing in list.iter() {
                if existing.is_null() {
                    continue;
                }
                let t = &*existing;
                if t.name == name_s && t.type_name != type_s {
                    return ptr::null_mut();
                }
            }
        }

        let topic = Box::new(ZeroDdsTopic {
            participant: p,
            name: name_s,
            type_name: type_s,
            qos: Mutex::new(qos),
        });
        let t = Box::into_raw(topic);
        if let Ok(mut list) = pp.topics.lock() {
            list.push(t);
        }
        t
    }
}

/// Loescht ein Topic.
///
/// # Safety
/// `p` und `t` muessen valide Handles sein und zueinander gehoeren.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_topic(
    p: *mut ZeroDdsDomainParticipant,
    t: *mut ZeroDdsTopic,
) -> c_int {
    if p.is_null() || t.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p+t NULL-checked above; t aus create_topic Box::into_raw.
    unsafe {
        let pp = &*p;
        if (*t).participant != p {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        if let Ok(mut list) = pp.topics.lock() {
            let len_before = list.len();
            list.retain(|x| *x != t);
            if list.len() == len_before {
                return ZeroDdsStatus::BadHandle as c_int;
            }
        }
        let _ = Box::from_raw(t);
    }
    ZeroDdsStatus::Ok as c_int
}

/// Findet ein bereits angelegtes Topic via Name. Liefert NULL wenn keins.
///
/// # Safety
/// `p`, `name` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_find_topic(
    p: *mut ZeroDdsDomainParticipant,
    name: *const c_char,
) -> *mut ZeroDdsTopic {
    if p.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; name per Caller-Pledge;
    // topics-Liste haelt nur valide Box-Pointer.
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

// ---------------------------------------------------------------------------
// Publisher
// ---------------------------------------------------------------------------

/// Erzeugt einen Publisher.
///
/// # Safety
/// `p` valide; `qos` darf NULL sein.
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
            suspended: Mutex::new(false),
        });
        let pp_ptr = Box::into_raw(pub_);
        if let Ok(mut list) = pp.publishers.lock() {
            list.push(pp_ptr);
        }
        pp_ptr
    }
}

/// Loescht einen Publisher.
///
/// # Safety
/// Beide Handles valide; Publisher darf keine DataWriter mehr enthalten.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_publisher(
    p: *mut ZeroDdsDomainParticipant,
    pubh: *mut ZeroDdsPublisher,
) -> c_int {
    if p.is_null() || pubh.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p+pubh NULL-checked above; pubh aus create_publisher.
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

/// Erzeugt einen Subscriber.
///
/// # Safety
/// `p` valide; `qos` darf NULL sein.
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
        let sub_ = Box::new(ZeroDdsSubscriber {
            participant: p,
            qos: Mutex::new(qos),
            default_dr_qos: Mutex::new(DataReaderQos::default()),
            datareaders: Mutex::new(Vec::new()),
        });
        let sptr = Box::into_raw(sub_);
        if let Ok(mut list) = pp.subscribers.lock() {
            list.push(sptr);
        }
        sptr
    }
}

/// Loescht einen Subscriber.
///
/// # Safety
/// Beide Handles valide; Subscriber darf keine DataReader mehr halten.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_subscriber(
    p: *mut ZeroDdsDomainParticipant,
    sub: *mut ZeroDdsSubscriber,
) -> c_int {
    if p.is_null() || sub.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p+sub NULL-checked above; sub aus create_subscriber.
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

/// Erzeugt ein ContentFilteredTopic.
///
/// # Safety
/// `p`, `name`, `related`, `filter_expression` valide.
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
    // per Caller-Pledge; parameters[0..param_count] muss valide sein wenn parameters != NULL.
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
        let cft = Box::new(ZeroDdsContentFilteredTopic {
            participant: p,
            related_topic: related,
            name: name_s,
            filter_expression: expr_s,
            parameters: Mutex::new(params),
        });
        Box::into_raw(cft)
    }
}

/// Loescht ein ContentFilteredTopic.
///
/// # Safety
/// Beide Handles valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_contentfilteredtopic(
    p: *mut ZeroDdsDomainParticipant,
    cft: *mut ZeroDdsContentFilteredTopic,
) -> c_int {
    if p.is_null() || cft.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p+cft NULL-checked above; cft aus create_contentfilteredtopic.
    unsafe {
        if (*cft).participant != p {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
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
/// `p` valide.
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
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Ignore Topic by InstanceHandle.
///
/// # Safety
/// `p` valide.
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
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Ignore Publication by InstanceHandle.
///
/// # Safety
/// `p` valide.
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
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

/// Ignore Subscription by InstanceHandle.
///
/// # Safety
/// `p` valide.
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
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

/// Domain-ID des Participant.
///
/// # Safety
/// `p` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_domain_id(p: *mut ZeroDdsDomainParticipant) -> u32 {
    if p.is_null() {
        return u32::MAX;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above.
    unsafe { (*p).domain_id }
}

/// Liveliness fuer alle vom Participant gehaltenen MANUAL_BY_PARTICIPANT-
/// Writers asserten.
///
/// # Safety
/// `p` valide.
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

/// Loescht alle vom Participant gehaltenen Topics, Publisher, Subscriber
/// rekursiv (Spec §2.2.2.2.1.10).
///
/// # Safety
/// `p` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_delete_contained_entities(
    p: *mut ZeroDdsDomainParticipant,
) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — p NULL-checked above; alle Listen halten nur
    // Pointer aus Box::into_raw der entsprechenden create_*-fns.
    unsafe {
        let pp = &*p;

        // Reihenfolge: erst Pub/Sub (incl. ihre DW/DR), dann Topics.
        let pubs: Vec<*mut ZeroDdsPublisher> = pp
            .publishers
            .lock()
            .map(|mut g| core::mem::take(&mut *g))
            .unwrap_or_default();
        for pub_ptr in pubs {
            if pub_ptr.is_null() {
                continue;
            }
            // delete_contained_entities auf Publisher: hier inline
            // (Publisher-FFI implementiert das in publisher_ffi.rs, aber
            // wir ziehen lokal ab).
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
    }
    ZeroDdsStatus::Ok as c_int
}

/// Prueft ob ein InstanceHandle zu einer Entity in diesem Participant
/// gehoert (Spec §2.2.2.2.1.11).
///
/// # Safety
/// `p` valide.
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
    let contains = unsafe { (*p).dp.contains_entity(InstanceHandle::from_raw(handle)) };
    if contains { 1 } else { 0 }
}

// ---------------------------------------------------------------------------
// Discovery-Listing
// ---------------------------------------------------------------------------

/// Liefert die InstanceHandles aller entdeckten Remote-Participants.
/// `out_handles[0..*out_count]` werden geschrieben, max `cap`.
///
/// # Safety
/// `p`, `out_handles`, `out_count` valide; `out_handles[0..cap]` writeable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_discovered_participants(
    p: *mut ZeroDdsDomainParticipant,
    out_handles: *mut u64,
    out_count: *mut usize,
    cap: usize,
) -> c_int {
    if p.is_null() || out_handles.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: see fn # Safety doc — p+out_handles+out_count NULL-checked above;
    // out_handles muss cap-Slots haben (Caller-Pledge).
    unsafe {
        let pp = &*p;
        let handles = pp.dp.get_discovered_participants();
        let n = handles.len().min(cap);
        let dst = slice::from_raw_parts_mut(out_handles, n);
        for (i, h) in handles.iter().take(n).enumerate() {
            dst[i] = h.as_raw();
        }
        *out_count = n;
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

    fn mk_participant(domain: u32) -> *mut ZeroDdsDomainParticipant {
        let f = zerodds_dpf_get_instance();
        // SAFETY: f aus dpf_get_instance, statisch valide.
        unsafe { zerodds_dpf_create_participant(f, domain, ptr::null()) }
    }

    fn drop_participant(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: p aus mk_participant; f statisch.
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
        // SAFETY: p aus mk; n+tn statisch.
        unsafe {
            let t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            assert!(!t.is_null());
            let f = zerodds_dp_find_topic(p, n.as_ptr());
            assert_eq!(f, t);
            let rc = zerodds_dp_delete_topic(p, t);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        }
        drop_participant(p);
    }

    #[test]
    fn topic_name_collision_with_different_type_returns_null() {
        let p = mk_participant(12);
        let n = c"X";
        let t1 = c"TypeA";
        let t2 = c"TypeB";
        // SAFETY: p aus mk; n+t1+t2 statisch.
        unsafe {
            let a = zerodds_dp_create_topic(p, n.as_ptr(), t1.as_ptr(), ptr::null());
            assert!(!a.is_null());
            let b = zerodds_dp_create_topic(p, n.as_ptr(), t2.as_ptr(), ptr::null());
            assert!(b.is_null(), "name+type collision must be rejected");
        }
        drop_participant(p);
    }

    #[test]
    fn create_delete_publisher_clean() {
        let p = mk_participant(13);
        // SAFETY: p aus mk.
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
        // SAFETY: p aus mk.
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
        // SAFETY: p aus mk; n+tn statisch; f statisch.
        unsafe {
            let _t = zerodds_dp_create_topic(p, n.as_ptr(), tn.as_ptr(), ptr::null());
            let _pubh = zerodds_dp_create_publisher(p, ptr::null());
            let _sub = zerodds_dp_create_subscriber(p, ptr::null());
            let rc = zerodds_dp_delete_contained_entities(p);
            assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
            // Nun darf delete_participant gehen.
            let rc2 = zerodds_dpf_delete_participant(f, p);
            assert_eq!(rc2, ZeroDdsStatus::Ok as c_int);
        }
    }

    #[test]
    fn domain_id_roundtrip() {
        let p = mk_participant(99);
        // SAFETY: p aus mk.
        assert_eq!(unsafe { zerodds_dp_get_domain_id(p) }, 99);
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
        // SAFETY: p aus mk; alle CStr+param-arr statisch/stack-lokal.
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
        // SAFETY: p aus mk.
        let rc = unsafe { zerodds_dp_ignore_participant(p, 12345) };
        // Valid handle path: rt may return Error if handle unknown — both Ok.
        assert!(rc == ZeroDdsStatus::Ok as c_int || rc == ZeroDdsStatus::Error as c_int);
        drop_participant(p);
    }
}
