// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DomainParticipantFactory C-FFI (Spec §2.2.2.2.2 + DDS-PSM-Cxx §7.5.1).

use alloc::boxed::Box;
use core::ffi::c_int;
use core::ptr;
use std::sync::Mutex;

use zerodds_dcps::factory::DomainParticipantFactory as DcpsFactory;
use zerodds_dcps::qos::{DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos};

use crate::entities::{ZeroDdsDomainParticipant, ZeroDdsDomainParticipantFactory};
use crate::qos_ffi::{
    ZeroDdsDomainParticipantFactoryQos, ZeroDdsDomainParticipantQos, dp_qos_from_c, dpf_qos_from_c,
};

/// Get singleton: returns the global factory pointer. Never NULL.
/// The caller must **not** free the pointer.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_dpf_get_instance() -> *const ZeroDdsDomainParticipantFactory {
    ZeroDdsDomainParticipantFactory::instance() as *const _
}

/// Creates a new DomainParticipant.
///
/// # Safety
/// `f` must come from `zerodds_dpf_get_instance` or be NULL.
/// `qos` may be NULL (default).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_create_participant(
    f: *const ZeroDdsDomainParticipantFactory,
    domain_id: u32,
    qos: *const ZeroDdsDomainParticipantQos,
) -> *mut ZeroDdsDomainParticipant {
    if f.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — f NULL-checked above; comes from get_instance
    // (static lifetime); qos NULL-tolerant.
    unsafe {
        let factory = &*f;
        let qos: DomainParticipantQos = if qos.is_null() {
            factory
                .default_participant_qos
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default()
        } else {
            dp_qos_from_c(qos)
        };

        let dp = match DcpsFactory::instance().create_participant(domain_id as i32, qos) {
            Ok(p) => p,
            Err(_) => return ptr::null_mut(),
        };
        let rt_clone = dp.runtime().cloned();

        let participant = Box::new(ZeroDdsDomainParticipant {
            dp,
            rt: rt_clone,
            domain_id,
            default_topic_qos: Mutex::new(TopicQos::default()),
            default_publisher_qos: Mutex::new(PublisherQos::default()),
            default_subscriber_qos: Mutex::new(SubscriberQos::default()),
            topics: Mutex::new(alloc::vec::Vec::new()),
            publishers: Mutex::new(alloc::vec::Vec::new()),
            subscribers: Mutex::new(alloc::vec::Vec::new()),
        });
        let p = Box::into_raw(participant);
        if let Ok(mut list) = factory.participants.lock() {
            list.push(p);
        }
        p
    }
}

/// Deletes a DomainParticipant.
///
/// # Safety
/// `f` and `p` must be valid handles. `p` must not have any
/// contained entities (topics, publishers, subscribers) at the time
/// of the call — the caller must call `delete_contained_entities`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_delete_participant(
    f: *const ZeroDdsDomainParticipantFactory,
    p: *mut ZeroDdsDomainParticipant,
) -> c_int {
    if f.is_null() || p.is_null() {
        return crate::ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — f+p NULL-checked above; p from
    // dpf_create_participant (Box::into_raw); contained-entities check before drop.
    unsafe {
        let factory = &*f;
        let pp = &*p;
        let has_topics = pp.topics.lock().map(|v| !v.is_empty()).unwrap_or(false);
        let has_pubs = pp.publishers.lock().map(|v| !v.is_empty()).unwrap_or(false);
        let has_subs = pp
            .subscribers
            .lock()
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if has_topics || has_pubs || has_subs {
            return crate::ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        if let Ok(mut list) = factory.participants.lock() {
            list.retain(|x| *x != p);
        }
        let _ = Box::from_raw(p);
    }
    crate::ZeroDdsStatus::Ok as c_int
}

/// Finds an active participant for the given domain ID. Returns
/// NULL if none exists.
///
/// # Safety
/// `f` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_lookup_participant(
    f: *const ZeroDdsDomainParticipantFactory,
    domain_id: u32,
) -> *mut ZeroDdsDomainParticipant {
    if f.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: see fn # Safety doc — f NULL-checked above; the participants list holds
    // only valid box pointers from create_participant.
    unsafe {
        let factory = &*f;
        if let Ok(list) = factory.participants.lock() {
            for &p in list.iter() {
                if p.is_null() {
                    continue;
                }
                if (*p).domain_id == domain_id {
                    return p;
                }
            }
        }
    }
    ptr::null_mut()
}

/// Get default participant QoS via a pass-through pointer to an
/// `Arc<...>`-internal snapshot. The caller may only read the pointer,
/// not free it (static lifetime of the singleton factory).
///
/// In this RC1 surface form the default QoS is returned as an opaque
/// `Arc<DomainParticipantQos>` — the full field getter/setter
/// comes with the `qos.rs` structs in a follow-up pass.
///
/// # Safety
/// `f` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_set_default_participant_qos(
    f: *const ZeroDdsDomainParticipantFactory,
    qos: *const ZeroDdsDomainParticipantQos,
) -> c_int {
    if f.is_null() {
        return crate::ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — f NULL-checked above; qos NULL-tolerant.
    unsafe {
        let factory = &*f;
        let new_qos: DomainParticipantQos = if qos.is_null() {
            DomainParticipantQos::default()
        } else {
            dp_qos_from_c(qos)
        };
        if let Ok(mut g) = factory.default_participant_qos.lock() {
            *g = new_qos;
        }
    }
    crate::ZeroDdsStatus::Ok as c_int
}

/// Returns a copy of the current default participant QoS in `out`.
/// As long as the `zerodds_DomainParticipantQos` struct is not defined in the
/// `qos.rs` layer, `out` is an opaque `void*`.
///
/// # Safety
/// `f` valid; `out` may be NULL (no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_get_default_participant_qos(
    f: *const ZeroDdsDomainParticipantFactory,
    out: *mut ZeroDdsDomainParticipantQos,
) -> c_int {
    if f.is_null() || out.is_null() {
        return crate::ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — f+out NULL-checked above.
    unsafe {
        let factory = &*f;
        let qos = factory
            .default_participant_qos
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        crate::qos_ffi::dp_qos_to_c(&qos, out)
    }
}

/// Sets the factory's own QoS (Spec §2.2.2.2.2.6).
///
/// # Safety
/// `f` valid; `qos` may be NULL (reset).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_set_qos(
    f: *const ZeroDdsDomainParticipantFactory,
    qos: *const ZeroDdsDomainParticipantFactoryQos,
) -> c_int {
    if f.is_null() {
        return crate::ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — f NULL-checked above; qos NULL-tolerant.
    unsafe {
        let factory = &*f;
        let new_qos = if qos.is_null() {
            zerodds_dcps::factory::DomainParticipantFactoryQos::default()
        } else {
            dpf_qos_from_c(qos)
        };
        if let Ok(mut g) = factory.factory_qos.lock() {
            *g = new_qos;
        }
    }
    crate::ZeroDdsStatus::Ok as c_int
}

/// Reads the factory's own QoS.
///
/// # Safety
/// `f`, `out` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_get_qos(
    f: *const ZeroDdsDomainParticipantFactory,
    out: *mut ZeroDdsDomainParticipantFactoryQos,
) -> c_int {
    if f.is_null() || out.is_null() {
        return crate::ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: see fn # Safety doc — f+out NULL-checked above.
    unsafe {
        let factory = &*f;
        let qos = factory.factory_qos.lock().map(|g| *g).unwrap_or_default();
        (*out).entity_factory.autoenable_created_entities = qos.autoenable_created_entities;
    }
    crate::ZeroDdsStatus::Ok as c_int
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn factory_singleton_returns_non_null_stable_ptr() {
        let f1 = zerodds_dpf_get_instance();
        let f2 = zerodds_dpf_get_instance();
        assert!(!f1.is_null());
        assert_eq!(f1, f2, "singleton must return stable pointer");
    }

    #[test]
    fn create_and_delete_participant_clean_lifecycle() {
        let f = zerodds_dpf_get_instance();
        // SAFETY: f from dpf_get_instance (static).
        unsafe {
            let p = zerodds_dpf_create_participant(f, 0, ptr::null());
            assert!(!p.is_null(), "participant must be created");
            let lookup = zerodds_dpf_lookup_participant(f, 0);
            assert_eq!(lookup, p, "lookup must return same handle");
            let rc = zerodds_dpf_delete_participant(f, p);
            assert_eq!(rc, crate::ZeroDdsStatus::Ok as c_int);
        }
    }

    #[test]
    fn delete_with_null_handles_returns_bad_handle() {
        let f = zerodds_dpf_get_instance();
        // SAFETY: f static; NULL is explicitly allowed in the delete_participant contract.
        unsafe {
            let rc = zerodds_dpf_delete_participant(f, ptr::null_mut());
            assert_eq!(rc, crate::ZeroDdsStatus::BadHandle as c_int);
            let rc2 = zerodds_dpf_delete_participant(ptr::null(), ptr::null_mut());
            assert_eq!(rc2, crate::ZeroDdsStatus::BadHandle as c_int);
        }
    }

    #[test]
    fn lookup_unknown_domain_returns_null() {
        let f = zerodds_dpf_get_instance();
        // SAFETY: f static.
        let p = unsafe { zerodds_dpf_lookup_participant(f, 99_999) };
        assert!(p.is_null());
    }
}
