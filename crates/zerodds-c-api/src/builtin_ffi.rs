// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Built-in Topic Data + Discovered Endpoints C-FFI (Spec §2.2.5).
//!
//! Liefert die 4 normativen Builtin-Topics:
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

// ---------------------------------------------------------------------------
// Strukturen
// ---------------------------------------------------------------------------

/// `DCPSParticipant`-Sample (Spec §2.2.5.1).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsParticipantBuiltinTopicData {
    /// 16-byte GUID.
    pub guid: [u8; 16],
    /// Pointer auf UserData-Bytes (statisch ueber `dp`-Lifetime im RC1
    /// Snapshot — Caller kopiert wenn er die Daten ueberlebt).
    pub user_data: *const u8,
    /// UserData-Laenge.
    pub user_data_len: usize,
}

/// `DCPSTopic`-Sample (Spec §2.2.5.2).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsTopicBuiltinTopicData {
    /// Synthetischer Topic-Key (16 byte).
    pub key: [u8; 16],
    /// Topic-Name als C-String (heap-allokiert; Caller muss
    /// `zerodds_string_free` rufen).
    pub name: *mut c_char,
    /// Type-Name (heap-allokiert; Caller muss `zerodds_string_free` rufen).
    pub type_name: *mut c_char,
    /// Durability-Kind (0=Volatile, 1=TransientLocal, 2=Transient, 3=Persistent).
    pub durability_kind: u32,
    /// Reliability-Kind (1=BestEffort, 2=Reliable).
    pub reliability_kind: u32,
}

/// `DCPSPublication`-Sample (Spec §2.2.5.3).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPublicationBuiltinTopicData {
    /// Endpoint-GUID (Writer).
    pub key: [u8; 16],
    /// Owning-Participant-GUID.
    pub participant_key: [u8; 16],
    /// Topic-Name (heap-allokiert; via `zerodds_string_free`).
    pub topic_name: *mut c_char,
    /// Type-Name (heap-allokiert).
    pub type_name: *mut c_char,
    /// Durability.
    pub durability_kind: u32,
    /// Reliability.
    pub reliability_kind: u32,
    /// Ownership (0=Shared, 1=Exclusive).
    pub ownership_kind: u32,
    /// Ownership-Strength.
    pub ownership_strength: i32,
    /// Liveliness-Lease in Sekunden.
    pub liveliness_lease_seconds: i32,
    /// Deadline-Period in Sekunden.
    pub deadline_seconds: i32,
    /// Lifespan-Duration in Sekunden.
    pub lifespan_seconds: i32,
}

/// `DCPSSubscription`-Sample (Spec §2.2.5.4).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsSubscriptionBuiltinTopicData {
    /// Endpoint-GUID (Reader).
    pub key: [u8; 16],
    /// Owning-Participant-GUID.
    pub participant_key: [u8; 16],
    /// Topic-Name (heap-allokiert).
    pub topic_name: *mut c_char,
    /// Type-Name (heap-allokiert).
    pub type_name: *mut c_char,
    /// Durability.
    pub durability_kind: u32,
    /// Reliability.
    pub reliability_kind: u32,
    /// Ownership.
    pub ownership_kind: u32,
    /// Liveliness-Lease in Sekunden.
    pub liveliness_lease_seconds: i32,
    /// Deadline-Period in Sekunden.
    pub deadline_seconds: i32,
}

// ---------------------------------------------------------------------------
// DP → Discovered Topics + Data
// ---------------------------------------------------------------------------

/// Liefert die InstanceHandles aller entdeckten Topics.
///
/// # Safety
/// `p`, `out`, `out_count` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_discovered_topics(
    p: *mut ZeroDdsDomainParticipant,
    out: *mut u64,
    out_count: *mut usize,
    cap: usize,
) -> c_int {
    if p.is_null() || out.is_null() || out_count.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*p };
    let handles = pp.dp.get_discovered_topics();
    let n = handles.len().min(cap);
    // SAFETY: cap-grosser Write-Buffer.
    let dst = unsafe { slice::from_raw_parts_mut(out, n) };
    for (i, h) in handles.iter().take(n).enumerate() {
        dst[i] = h.as_raw();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { *out_count = n };
    ZeroDdsStatus::Ok as c_int
}

/// Liefert die `ParticipantBuiltinTopicData` zu einem InstanceHandle.
///
/// # Safety
/// `p`, `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_discovered_participant_data(
    p: *mut ZeroDdsDomainParticipant,
    handle: u64,
    out: *mut ZeroDdsParticipantBuiltinTopicData,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*p };
    let data = match pp
        .dp
        .get_discovered_participant_data(InstanceHandle::from_raw(handle))
    {
        Ok(d) => d,
        Err(_) => return ZeroDdsStatus::BadParameter as c_int,
    };
    let mut guid = [0u8; 16];
    guid.copy_from_slice(&data.key.to_bytes());
    // UserData wird als Box<[u8]> in einen leak'ed Slice gehievt; Caller
    // muss via `zerodds_builtin_userdata_free` zurueckgeben.
    let bytes = data.user_data.into_boxed_slice();
    let len = bytes.len();
    let p_data = if len == 0 {
        ptr::null()
    } else {
        Box::leak(bytes).as_ptr()
    };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        *out = ZeroDdsParticipantBuiltinTopicData {
            guid,
            user_data: p_data,
            user_data_len: len,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// Gibt einen vorher allokierten UserData-Slice zurueck.
///
/// # Safety
/// `p[0..len]` muss aus `dp_get_discovered_participant_data` stammen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_userdata_free(p: *const u8, len: usize) {
    if p.is_null() || len == 0 {
        return;
    }
    // SAFETY: Caller-Kontrakt: aus Box::leak(box<[u8]>) mit gleichem len.
    let _ = unsafe { Box::from_raw(slice::from_raw_parts_mut(p as *mut u8, len)) };
}

/// Liefert die `TopicBuiltinTopicData` zu einem InstanceHandle.
///
/// # Safety
/// `p`, `out` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_discovered_topic_data(
    p: *mut ZeroDdsDomainParticipant,
    handle: u64,
    out: *mut ZeroDdsTopicBuiltinTopicData,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*p };
    let data = match pp
        .dp
        .get_discovered_topic_data(InstanceHandle::from_raw(handle))
    {
        Ok(d) => d,
        Err(_) => return ZeroDdsStatus::BadParameter as c_int,
    };
    let mut key = [0u8; 16];
    key.copy_from_slice(&data.key.to_bytes());
    let name_c = CString::new(data.name.as_bytes())
        .unwrap_or_default()
        .into_raw();
    let type_c = CString::new(data.type_name.as_bytes())
        .unwrap_or_default()
        .into_raw();
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        *out = ZeroDdsTopicBuiltinTopicData {
            key,
            name: name_c,
            type_name: type_c,
            durability_kind: data.durability as u32,
            reliability_kind: data.reliability as u32,
        };
    }
    ZeroDdsStatus::Ok as c_int
}

/// Anzahl entdeckter Publications (Spec §2.2.2.2.1.13).
///
/// # Safety
/// `p` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_discovered_publications_count(
    p: *mut ZeroDdsDomainParticipant,
) -> usize {
    if p.is_null() {
        return 0;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*p };
    pp.dp.discovered_publications_count()
}

/// Anzahl entdeckter Subscriptions.
///
/// # Safety
/// `p` valide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_discovered_subscriptions_count(
    p: *mut ZeroDdsDomainParticipant,
) -> usize {
    if p.is_null() {
        return 0;
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let pp = unsafe { &*p };
    pp.dp.discovered_subscriptions_count()
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
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_dpf_create_participant(f, domain, ptr::null()) }
    }

    fn cleanup(p: *mut ZeroDdsDomainParticipant) {
        let f = zerodds_dpf_get_instance();
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe {
            zerodds_dp_delete_contained_entities(p);
            zerodds_dpf_delete_participant(f, p);
        }
    }

    #[test]
    fn discovered_topics_empty_initially() {
        let p = mk(61);
        let mut buf = [0u64; 16];
        let mut count = 0usize;
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dp_get_discovered_topics(p, buf.as_mut_ptr(), &mut count, 16) };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(count, 0);
        cleanup(p);
    }

    #[test]
    fn discovered_publications_count_starts_zero() {
        let p = mk(62);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert_eq!(unsafe { zerodds_dp_discovered_publications_count(p) }, 0);
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        assert_eq!(unsafe { zerodds_dp_discovered_subscriptions_count(p) }, 0);
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
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let rc = unsafe { zerodds_dp_get_discovered_participant_data(p, 0xDEAD, &mut data) };
        assert_eq!(rc, ZeroDdsStatus::BadParameter as c_int);
        cleanup(p);
    }
}
