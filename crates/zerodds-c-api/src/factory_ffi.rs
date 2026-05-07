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

/// Get-Singleton: liefert den globalen Factory-Pointer. Niemals NULL.
/// Caller darf den Pointer **nicht** freigeben.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_dpf_get_instance() -> *const ZeroDdsDomainParticipantFactory {
    ZeroDdsDomainParticipantFactory::instance() as *const _
}

/// Erzeugt einen neuen DomainParticipant.
///
/// # Safety
/// `f` muss aus `zerodds_dpf_get_instance` stammen oder NULL sein.
/// `qos` darf NULL sein (default).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_create_participant(
    f: *const ZeroDdsDomainParticipantFactory,
    domain_id: u32,
    qos: *const ZeroDdsDomainParticipantQos,
) -> *mut ZeroDdsDomainParticipant {
    if f.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: f ist non-null und stammt aus get_instance (statische Lifetime).
    let factory = unsafe { &*f };

    // QoS aus Caller wenn non-NULL, sonst Factory-Default.
    let qos: DomainParticipantQos = if qos.is_null() {
        factory
            .default_participant_qos
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    } else {
        // SAFETY: NULL-Check oben.
        unsafe { dp_qos_from_c(qos) }
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

/// Loescht einen DomainParticipant.
///
/// # Safety
/// `f` und `p` muessen valide Handles sein. `p` darf nicht zum
/// Zeitpunkt des Aufrufs noch enthaltene Entities (Topics, Publisher,
/// Subscriber) haben — Caller muss `delete_contained_entities` rufen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_delete_participant(
    f: *const ZeroDdsDomainParticipantFactory,
    p: *mut ZeroDdsDomainParticipant,
) -> c_int {
    if f.is_null() || p.is_null() {
        return crate::ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL-checks.
    let factory = unsafe { &*f };

    // Pruefe ob noch Topics/Pub/Sub enthalten — Spec §2.2.2.2.1.5
    // verlangt PreconditionNotMet wenn ja.
    {
        // SAFETY: p non-null, gueltige Box.
        let pp = unsafe { &*p };
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
    }

    // Aus Factory-Registry entfernen.
    if let Ok(mut list) = factory.participants.lock() {
        list.retain(|x| *x != p);
    }

    // SAFETY: Caller hat den Pointer von dpf_create_participant.
    let _ = unsafe { Box::from_raw(p) };
    crate::ZeroDdsStatus::Ok as c_int
}

/// Findet einen aktiven Participant zu der gegebenen Domain-ID. Liefert
/// NULL wenn keiner existiert.
///
/// # Safety
/// `f` muss valide sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_lookup_participant(
    f: *const ZeroDdsDomainParticipantFactory,
    domain_id: u32,
) -> *mut ZeroDdsDomainParticipant {
    if f.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: f non-null.
    let factory = unsafe { &*f };
    if let Ok(list) = factory.participants.lock() {
        for &p in list.iter() {
            if p.is_null() {
                continue;
            }
            // SAFETY: list haelt nur valide Box-Pointer aus create_participant.
            let pp = unsafe { &*p };
            if pp.domain_id == domain_id {
                return p;
            }
        }
    }
    ptr::null_mut()
}

/// Get-Default-Participant-QoS via Pass-Through-Pointer auf einen
/// `Arc<...>`-internen Snapshot. Caller darf den Pointer nur lesen,
/// nicht freigeben (statische Lifetime der Singleton-Factory).
///
/// In dieser RC1-Surface-Form wird die Default-QoS als opaker
/// `Arc<DomainParticipantQos>` geliefert — die volle Field-getter/setter
/// kommt mit den `qos.rs`-Strukturen in Folge-Pass.
///
/// # Safety
/// `f` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_set_default_participant_qos(
    f: *const ZeroDdsDomainParticipantFactory,
    qos: *const ZeroDdsDomainParticipantQos,
) -> c_int {
    if f.is_null() {
        return crate::ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: f non-null.
    let factory = unsafe { &*f };
    let new_qos: DomainParticipantQos = if qos.is_null() {
        DomainParticipantQos::default()
    } else {
        // SAFETY: NULL-Check.
        unsafe { dp_qos_from_c(qos) }
    };
    if let Ok(mut g) = factory.default_participant_qos.lock() {
        *g = new_qos;
    }
    crate::ZeroDdsStatus::Ok as c_int
}

/// Gibt eine Kopie der aktuellen Default-Participant-QoS in `out` zurueck.
/// Solange die `zerodds_DomainParticipantQos`-Struktur nicht in der
/// `qos.rs`-Schicht definiert ist, ist `out` ein opaker `void*`.
///
/// # Safety
/// `f` valide; `out` darf NULL sein (no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_get_default_participant_qos(
    f: *const ZeroDdsDomainParticipantFactory,
    out: *mut ZeroDdsDomainParticipantQos,
) -> c_int {
    if f.is_null() || out.is_null() {
        return crate::ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let factory = unsafe { &*f };
    let qos = factory
        .default_participant_qos
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    // SAFETY: out non-null.
    unsafe { crate::qos_ffi::dp_qos_to_c(&qos, out) }
}

/// Factory-eigene QoS setzen (Spec §2.2.2.2.2.6).
///
/// # Safety
/// `f` valide; `qos` darf NULL sein (Reset).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_set_qos(
    f: *const ZeroDdsDomainParticipantFactory,
    qos: *const ZeroDdsDomainParticipantFactoryQos,
) -> c_int {
    if f.is_null() {
        return crate::ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let factory = unsafe { &*f };
    let new_qos = if qos.is_null() {
        zerodds_dcps::factory::DomainParticipantFactoryQos::default()
    } else {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { dpf_qos_from_c(qos) }
    };
    if let Ok(mut g) = factory.factory_qos.lock() {
        *g = new_qos;
    }
    crate::ZeroDdsStatus::Ok as c_int
}

/// Factory-eigene QoS lesen.
///
/// # Safety
/// `f`, `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dpf_get_qos(
    f: *const ZeroDdsDomainParticipantFactory,
    out: *mut ZeroDdsDomainParticipantFactoryQos,
) -> c_int {
    if f.is_null() || out.is_null() {
        return crate::ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let factory = unsafe { &*f };
    let qos = factory.factory_qos.lock().map(|g| *g).unwrap_or_default();
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
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
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let p = unsafe { zerodds_dpf_create_participant(f, 0, ptr::null()) };
        assert!(!p.is_null(), "participant must be created");

        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let lookup = unsafe { zerodds_dpf_lookup_participant(f, 0) };
        assert_eq!(lookup, p, "lookup must return same handle");

        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dpf_delete_participant(f, p) };
        assert_eq!(rc, crate::ZeroDdsStatus::Ok as c_int);
    }

    #[test]
    fn delete_with_null_handles_returns_bad_handle() {
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dpf_delete_participant(f, ptr::null_mut()) };
        assert_eq!(rc, crate::ZeroDdsStatus::BadHandle as c_int);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc2 = unsafe { zerodds_dpf_delete_participant(ptr::null(), ptr::null_mut()) };
        assert_eq!(rc2, crate::ZeroDdsStatus::BadHandle as c_int);
    }

    #[test]
    fn lookup_unknown_domain_returns_null() {
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let p = unsafe { zerodds_dpf_lookup_participant(f, 99_999) };
        assert!(p.is_null());
    }
}
