// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Instance operations that take a handle where the older functions take a
//! key: the complete key hash of an instance a writer knows, so that
//! `dispose_instance(handle)` and `unregister_instance(handle)` of the C++
//! API (DDS-PSM-Cxx 1.0 §7.5.4 `TDataWriter`) reach `zerodds_dw_dispose`
//! with the key of the instance.

use core::ffi::c_int;

use zerodds_dcps::instance_handle::InstanceHandle;

use crate::ZeroDdsStatus;
use crate::entities::{ZeroDdsDataReader, ZeroDdsDataWriter};

/// Writes the 16-byte key hash of the instance `handle` names to `out_key`.
/// `ZERODDS_BAD_PARAMETER` for a handle this writer has not registered
/// (DDS 1.4 §2.2.2.4.2.9 `dispose`: "the InstanceHandle_t ... was not
/// registered").
///
/// # Safety
/// `dw` is a live DataWriter handle, `out_key` writable for 16 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_instance_key_hash(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    out_key: *mut u8,
) -> c_int {
    if dw.is_null() || out_key.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    let writer = unsafe { &*dw };
    match writer
        .instances
        .key_hash_for_handle(InstanceHandle::from_raw(handle))
    {
        Some(key) => {
            // SAFETY: `out_key` is writable for 16 bytes per the contract.
            unsafe { core::ptr::copy_nonoverlapping(key.as_ptr(), out_key, 16) };
            ZeroDdsStatus::Ok as c_int
        }
        None => ZeroDdsStatus::BadParameter as c_int,
    }
}

/// Writes the 16-byte key hash of the sample `payload` holds to `out_key_hash`, for the type
/// whose type objects `zerodds_dr_set_type_objects` gave the reader. `xcdr2` selects XCDR2
/// (else XCDR1) and `big_endian` the byte order of `payload`, which carries no encapsulation
/// header. DDS 1.4 §2.2.2.5.3.27 `lookup_instance` takes the sample whose key names the
/// instance. `ZERODDS_PRECONDITION_NOT_MET` for a reader without type objects or whose type
/// has no key; `ZERODDS_BAD_PARAMETER` for a payload that does not decode as that type.
///
/// # Safety
/// `dr` is a live DataReader handle, `payload[0..len]` readable when `len > 0`,
/// `out_key_hash` writable for 16 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_sample_key_hash(
    dr: *mut ZeroDdsDataReader,
    payload: *const u8,
    len: usize,
    xcdr2: c_int,
    big_endian: c_int,
    out_key_hash: *mut u8,
) -> c_int {
    if dr.is_null() || out_key_hash.is_null() || (payload.is_null() && len > 0) {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: non-null, live per the caller's contract; `len` readable bytes.
    let (reader, bytes) = unsafe {
        let bytes: &[u8] = if len == 0 {
            &[]
        } else {
            core::slice::from_raw_parts(payload, len)
        };
        (&*dr, bytes)
    };
    let support = match reader.query_type.lock() {
        Ok(slot) => slot.clone(),
        Err(_) => return ZeroDdsStatus::Error as c_int,
    };
    let Some(support) = support else {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    };
    if !support.is_keyed() {
        return ZeroDdsStatus::PreconditionNotMet as c_int;
    }
    let representation = if xcdr2 != 0 { 2 } else { 0 };
    let Ok(sample) = support.read_body(bytes, representation, big_endian != 0) else {
        return ZeroDdsStatus::BadParameter as c_int;
    };
    match support.key_material(&sample) {
        Some((key, _holder)) => {
            // SAFETY: `out_key_hash` is writable for 16 bytes per the contract.
            unsafe { core::ptr::copy_nonoverlapping(key.as_ptr(), out_key_hash, 16) };
            ZeroDdsStatus::Ok as c_int
        }
        None => ZeroDdsStatus::PreconditionNotMet as c_int,
    }
}
