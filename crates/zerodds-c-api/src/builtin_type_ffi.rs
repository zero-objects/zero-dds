// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The built-in types of DDS-XTypes 1.3 §7.6.5 and Annex E in the C ABI.
//!
//! `DDS::String`, `DDS::KeyedString`, `DDS::Bytes` and `DDS::KeyedBytes`
//! are topic types every participant knows. A C program writes and reads
//! them without encoding anything itself:
//!
//! | Annex E | C |
//! |---|---|
//! | the four types | `ZERODDS_BUILTIN_STRING` … `ZERODDS_BUILTIN_KEYED_BYTES` |
//! | `create_topic` / `create_datawriter` / `create_datareader` over one | [`zerodds_dp_create_builtin_topic`], [`zerodds_pub_create_builtin_datawriter`], [`zerodds_sub_create_builtin_datareader`] |
//! | `StringDataWriter::write` | [`zerodds_string_writer_write`] |
//! | the operations of `KeyedStringDataWriter` | `zerodds_keyed_string_writer_*` |
//! | `BytesDataWriter::write_w_bytes` | [`zerodds_bytes_writer_write_w_bytes`] |
//! | the operations of `KeyedBytesDataWriter` | `zerodds_keyed_bytes_writer_*` |
//! | `get_type_name` and `register_type` of the four TypeSupports | [`zerodds_builtin_type_support_get_type_name`], [`zerodds_dp_register_builtin_type`] |
//! | `take`/`read` of the four readers | `zerodds_*_reader_take_next`, `zerodds_*_reader_read_next` |
//!
//! The writers and readers are the ordinary [`crate::entities::ZeroDdsDataWriter`] and
//! [`crate::entities::ZeroDdsDataReader`] of a topic whose type name is that of the built-in
//! type; the wire form is the appendable one of Annex E, XCDR2 on the
//! way out and either representation on the way in.
//!
//! A string or an array returned through an out-parameter belongs to the
//! caller: [`zerodds_builtin_string_free`] and
//! [`zerodds_builtin_bytes_free`] release them.

use alloc::ffi::CString;
use alloc::vec::Vec;
use core::ffi::{c_char, c_int};
use core::ptr;
use core::slice;
use std::ffi::CStr;

use zerodds_dcps::DdsType;
use zerodds_dcps::builtin_type_set::{
    NAME_DDS_BYTES, NAME_DDS_KEYED_BYTES, NAME_DDS_KEYED_STRING, NAME_DDS_STRING,
};
use zerodds_dcps::builtin_types::{DdsBytes, DdsKeyedBytes, DdsKeyedString, DdsString};
use zerodds_dcps::dds_type::PlainCdr2BeKeyHolder;

use crate::ZeroDdsStatus;
use crate::entities::{
    ZeroDdsDataReader, ZeroDdsDataWriter, ZeroDdsDomainParticipant, ZeroDdsPublisher,
    ZeroDdsSubscriber, ZeroDdsTopic,
};
use crate::publisher_ffi::{
    zerodds_dw_write, zerodds_dw_write_w_timestamp, zerodds_pub_create_datawriter_typed_wire,
};
use crate::qos_ffi::{ZeroDdsDataReaderQos, ZeroDdsDataWriterQos, ZeroDdsTopicQos};
use crate::subscriber_ffi::{
    ZeroDdsSampleArray, ZeroDdsSampleInfo, zerodds_dr_read, zerodds_dr_return_loan,
    zerodds_dr_take, zerodds_sub_create_datareader_typed,
};

/// `DDS::String` (§7.6.5.1).
pub const ZERODDS_BUILTIN_STRING: c_int = 0;
/// `DDS::KeyedString` (§7.6.5.2).
pub const ZERODDS_BUILTIN_KEYED_STRING: c_int = 1;
/// `DDS::Bytes` (§7.6.5.3).
pub const ZERODDS_BUILTIN_BYTES: c_int = 2;
/// `DDS::KeyedBytes` (§7.6.5.4).
pub const ZERODDS_BUILTIN_KEYED_BYTES: c_int = 3;

fn ok() -> c_int {
    ZeroDdsStatus::Ok as c_int
}

fn bad_parameter() -> c_int {
    ZeroDdsStatus::BadParameter as c_int
}

/// The physical type name of a kind.
fn type_name_of(kind: c_int) -> Option<&'static str> {
    match kind {
        ZERODDS_BUILTIN_STRING => Some(NAME_DDS_STRING),
        ZERODDS_BUILTIN_KEYED_STRING => Some(NAME_DDS_KEYED_STRING),
        ZERODDS_BUILTIN_BYTES => Some(NAME_DDS_BYTES),
        ZERODDS_BUILTIN_KEYED_BYTES => Some(NAME_DDS_KEYED_BYTES),
        _ => None,
    }
}

/// The COMPLETE `TypeObject` of a built-in type, serialized, and whether
/// the type is keyed.
fn type_object_of(type_name: &str) -> Option<(Vec<u8>, bool)> {
    let (object, keyed) = match type_name {
        NAME_DDS_STRING => (DdsString::type_object(), false),
        NAME_DDS_KEYED_STRING => (DdsKeyedString::type_object(), true),
        NAME_DDS_BYTES => (DdsBytes::type_object(), false),
        NAME_DDS_KEYED_BYTES => (DdsKeyedBytes::type_object(), true),
        _ => return None,
    };
    Some((object?.to_bytes_le().ok()?, keyed))
}

/// A C string as `&str`.
///
/// # Safety
/// `p` is NULL or a NUL-terminated string.
unsafe fn text<'a>(p: *const c_char) -> Result<&'a str, c_int> {
    if p.is_null() {
        return Err(bad_parameter());
    }
    // SAFETY: non-NULL, NUL-terminated per the caller's contract.
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map_err(|_| ZeroDdsStatus::InvalidUtf8 as c_int)
}

/// A string for the caller, to be released by
/// [`zerodds_builtin_string_free`]. A string with an interior NUL cannot
/// be one.
fn give_string(value: &str) -> Result<*mut c_char, c_int> {
    CString::new(value)
        .map(CString::into_raw)
        .map_err(|_| bad_parameter())
}

/// An array for the caller, to be released by
/// [`zerodds_builtin_bytes_free`].
fn give_bytes(value: &[u8]) -> (*mut u8, usize) {
    if value.is_empty() {
        return (ptr::null_mut(), 0);
    }
    let boxed: alloc::boxed::Box<[u8]> = value.into();
    let len = boxed.len();
    (alloc::boxed::Box::into_raw(boxed).cast::<u8>(), len)
}

/// `bytes[offset .. offset + length]`, the way the `write_w_bytes` family
/// of Annex E names them.
///
/// # Safety
/// `bytes[0..bytes_len]` is readable, or `bytes` is NULL with a zero length.
unsafe fn octets<'a>(
    bytes: *const u8,
    bytes_len: usize,
    offset: i32,
    length: i32,
) -> Result<&'a [u8], c_int> {
    if bytes.is_null() && bytes_len > 0 {
        return Err(bad_parameter());
    }
    let all: &[u8] = if bytes_len == 0 {
        &[]
    } else {
        // SAFETY: non-NULL and `bytes_len` readable per the caller.
        unsafe { slice::from_raw_parts(bytes, bytes_len) }
    };
    let range = usize::try_from(offset)
        .ok()
        .zip(usize::try_from(length).ok())
        .and_then(|(offset, length)| Some(offset..offset.checked_add(length)?));
    range
        .and_then(|range| all.get(range))
        .ok_or_else(bad_parameter)
}

fn encode<T: DdsType>(sample: &T) -> Result<Vec<u8>, c_int> {
    let mut out = Vec::new();
    sample
        .encode(&mut out)
        .map_err(|_| ZeroDdsStatus::Error as c_int)?;
    Ok(out)
}

/// Releases a string a function of this module returned. NULL is allowed.
///
/// # Safety
/// `value` came from a function of this module and was not released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_string_free(value: *mut c_char) {
    if !value.is_null() {
        // SAFETY: from `CString::into_raw`, per the contract.
        drop(unsafe { CString::from_raw(value) });
    }
}

/// Releases an array a function of this module returned. NULL is allowed.
///
/// # Safety
/// `bytes` and `len` are what a function of this module returned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_builtin_bytes_free(bytes: *mut u8, len: usize) {
    if !bytes.is_null() && len > 0 {
        // SAFETY: from `Box<[u8]>::into_raw` of exactly `len` octets.
        drop(unsafe { alloc::boxed::Box::from_raw(slice::from_raw_parts_mut(bytes, len)) });
    }
}

// ---------------------------------------------------------------------------
// TypeSupport
// ---------------------------------------------------------------------------

/// `get_type_name` of `StringTypeSupport`, `KeyedStringTypeSupport`,
/// `BytesTypeSupport` or `KeyedBytesTypeSupport` (Annex E): `kind` is one of
/// the four `ZERODDS_BUILTIN_*` constants. The string is static and is not
/// released; NULL for any other `kind`.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_builtin_type_support_get_type_name(kind: c_int) -> *const c_char {
    match kind {
        ZERODDS_BUILTIN_STRING => c"DDS::String".as_ptr(),
        ZERODDS_BUILTIN_KEYED_STRING => c"DDS::KeyedString".as_ptr(),
        ZERODDS_BUILTIN_BYTES => c"DDS::Bytes".as_ptr(),
        ZERODDS_BUILTIN_KEYED_BYTES => c"DDS::KeyedBytes".as_ptr(),
        _ => ptr::null(),
    }
}

/// `register_type(participant, type_name)` of the TypeSupport of a built-in
/// type (DDS 1.4 §2.2.2.3.6, Annex E): registers the type under `type_name`,
/// or under `get_type_name` where `type_name` is NULL. The type is the same
/// one the participant already has under its own name; registering it again
/// is not an error.
///
/// Returns `ZERODDS_OK`; `BadHandle` for a NULL participant;
/// `BadParameter` for a `kind` that is none of the four or a name that is no
/// name; `PreconditionNotMet` when another type is registered under the
/// name; `AlreadyDeleted`.
///
/// # Safety
/// `p` is a live participant; `type_name` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_register_builtin_type(
    p: *mut ZeroDdsDomainParticipant,
    kind: c_int,
    type_name: *const c_char,
) -> c_int {
    use zerodds_dcps::builtin_types::{
        BytesTypeSupport, KeyedBytesTypeSupport, KeyedStringTypeSupport, StringTypeSupport,
        TypeSupport,
    };
    use zerodds_dcps::error::DdsError;
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let name = if type_name.is_null() {
        None
    } else {
        // SAFETY: non-NULL and NUL-terminated per the contract.
        match unsafe { text(type_name) } {
            Ok(name) => Some(name),
            Err(status) => return status,
        }
    };
    let support: &dyn TypeSupport = match kind {
        ZERODDS_BUILTIN_STRING => &StringTypeSupport,
        ZERODDS_BUILTIN_KEYED_STRING => &KeyedStringTypeSupport,
        ZERODDS_BUILTIN_BYTES => &BytesTypeSupport,
        ZERODDS_BUILTIN_KEYED_BYTES => &KeyedBytesTypeSupport,
        _ => return bad_parameter(),
    };
    // SAFETY: live per the contract.
    let participant = unsafe { &(*p).dp };
    match support.register_type(participant, name) {
        Ok(()) => ok(),
        Err(DdsError::BadParameter { .. }) => bad_parameter(),
        Err(DdsError::PreconditionNotMet { .. }) => ZeroDdsStatus::PreconditionNotMet as c_int,
        Err(DdsError::AlreadyDeleted) => ZeroDdsStatus::AlreadyDeleted as c_int,
        Err(_) => ZeroDdsStatus::Error as c_int,
    }
}

// ---------------------------------------------------------------------------
// Topic, writer, reader
// ---------------------------------------------------------------------------

/// A topic of a built-in type: `kind` is one of the four
/// `ZERODDS_BUILTIN_*` constants. NULL for any other, and as
/// [`crate::participant_ffi::zerodds_dp_create_topic`] for the rest.
///
/// # Safety
/// As `zerodds_dp_create_topic`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_create_builtin_topic(
    p: *mut ZeroDdsDomainParticipant,
    name: *const c_char,
    kind: c_int,
    qos: *const ZeroDdsTopicQos,
) -> *mut ZeroDdsTopic {
    let Some(type_name) = type_name_of(kind) else {
        return ptr::null_mut();
    };
    let Ok(type_name) = CString::new(type_name) else {
        return ptr::null_mut();
    };
    // SAFETY: forwards the caller's contract.
    unsafe { crate::participant_ffi::zerodds_dp_create_topic(p, name, type_name.as_ptr(), qos) }
}

/// A DataWriter over a topic of a built-in type: it advertises the
/// type's `TypeObject` and writes the appendable form of Annex E in
/// XCDR2. NULL when the topic is not one of the four.
///
/// # Safety
/// As `zerodds_pub_create_datawriter`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_create_builtin_datawriter(
    pub_: *mut ZeroDdsPublisher,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataWriterQos,
) -> *mut ZeroDdsDataWriter {
    if pub_.is_null() || topic.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: `topic` is non-NULL and live per the contract.
    let Some((object, keyed)) = type_object_of(unsafe { &(*topic).type_name }) else {
        return ptr::null_mut();
    };
    // SAFETY: forwards the caller's contract; the object bytes are ours.
    unsafe {
        zerodds_pub_create_datawriter_typed_wire(
            pub_,
            topic,
            qos,
            object.as_ptr(),
            object.len(),
            c_int::from(keyed),
            1,
            2,
        )
    }
}

/// A DataReader over a topic of a built-in type. NULL when the topic is
/// not one of the four.
///
/// # Safety
/// As `zerodds_sub_create_datareader`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_create_builtin_datareader(
    sub: *mut ZeroDdsSubscriber,
    topic: *mut ZeroDdsTopic,
    qos: *const ZeroDdsDataReaderQos,
) -> *mut ZeroDdsDataReader {
    if sub.is_null() || topic.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: `topic` is non-NULL and live per the contract.
    let Some((object, _)) = type_object_of(unsafe { &(*topic).type_name }) else {
        return ptr::null_mut();
    };
    // SAFETY: forwards the caller's contract; the object bytes are ours.
    unsafe { zerodds_sub_create_datareader_typed(sub, topic, qos, object.as_ptr(), object.len()) }
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

/// The instance handle of the key of `sample`, nil when it is not
/// registered.
///
/// # Safety
/// `dw` is live.
unsafe fn handle_of<T: DdsType>(dw: *mut ZeroDdsDataWriter, sample: &T) -> u64 {
    let Some(hash) = sample.compute_key_hash() else {
        return 0;
    };
    // SAFETY: live per the contract.
    unsafe { &*dw }
        .instances
        .lookup(&hash)
        .map_or(0, |handle| handle.as_raw())
}

/// The instance of `sample` in the writer's tracker, with its key holder
/// remembered so that `get_key_value_w_key` can answer for it.
///
/// # Safety
/// `dw` is live.
unsafe fn register_with_holder<T: DdsType>(
    dw: *mut ZeroDdsDataWriter,
    sample: &T,
) -> Result<u64, c_int> {
    let Some(hash) = sample.compute_key_hash() else {
        return Err(bad_parameter());
    };
    // SAFETY: live per the contract.
    let writer = unsafe { &*dw };
    if let Some(known) = writer.instances.lookup(&hash) {
        return Ok(known.as_raw());
    }
    let limit = writer
        .qos
        .lock()
        .map_err(|_| ZeroDdsStatus::Error as c_int)?
        .resource_limits
        .max_instances;
    if limit >= 0 && writer.instances.len() >= limit as usize {
        return Err(ZeroDdsStatus::OutOfResources as c_int);
    }
    let mut holder = PlainCdr2BeKeyHolder::new();
    sample.encode_key_holder_be(&mut holder);
    Ok(writer
        .instances
        .register(hash, holder.as_bytes().to_vec(), None)
        .as_raw())
}

/// `write` and `write_w_timestamp` of any of the four types. The handle of
/// a write is nil or the one of the sample's instance (DDS 1.4
/// §2.2.2.4.2.11).
///
/// # Safety
/// `dw` is live.
unsafe fn write_sample<T: DdsType>(
    dw: *mut ZeroDdsDataWriter,
    sample: &T,
    handle: u64,
    stamp: Option<(i32, u32)>,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    if T::HAS_KEY {
        // SAFETY: non-NULL and live per the contract.
        if handle != 0 && unsafe { handle_of(dw, sample) } != handle {
            return bad_parameter();
        }
        // SAFETY: as above.
        if let Err(status) = unsafe { register_with_holder(dw, sample) } {
            return status;
        }
    }
    let bytes = match encode(sample) {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    // SAFETY: `bytes` is a live buffer of its length.
    unsafe {
        match stamp {
            None => zerodds_dw_write(dw, bytes.as_ptr(), bytes.len(), handle),
            Some((sec, nanosec)) => {
                zerodds_dw_write_w_timestamp(dw, bytes.as_ptr(), bytes.len(), handle, sec, nanosec)
            }
        }
    }
}

/// `StringDataWriter::write` (Annex E): writes `value`.
///
/// # Safety
/// `dw` is a live writer over a `DDS::String` topic; `value` is a
/// NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_string_writer_write(
    dw: *mut ZeroDdsDataWriter,
    value: *const c_char,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe {
        match text(value) {
            Ok(value) => write_sample(dw, &DdsString::new(value), 0, None),
            Err(status) => status,
        }
    }
}

/// `StringDataWriter::write_w_timestamp`.
///
/// # Safety
/// As [`zerodds_string_writer_write`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_string_writer_write_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    value: *const c_char,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe {
        match text(value) {
            Ok(value) => write_sample(dw, &DdsString::new(value), 0, Some((ts_sec, ts_nanosec))),
            Err(status) => status,
        }
    }
}

/// `KeyedStringDataWriter::write_string_w_key`: writes `value` under
/// `key`. `handle` is 0 or the handle of the key's instance.
///
/// # Safety
/// `dw` is a live writer over a `DDS::KeyedString` topic; `key` and
/// `value` are NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_writer_write_string_w_key(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    value: *const c_char,
    handle: u64,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe {
        match (text(key), text(value)) {
            (Ok(key), Ok(value)) => {
                write_sample(dw, &DdsKeyedString::new(key, value), handle, None)
            }
            (Err(status), _) | (_, Err(status)) => status,
        }
    }
}

/// `KeyedStringDataWriter::write_string_w_key_w_timestamp`.
///
/// # Safety
/// As [`zerodds_keyed_string_writer_write_string_w_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_writer_write_string_w_key_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    value: *const c_char,
    handle: u64,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe {
        match (text(key), text(value)) {
            (Ok(key), Ok(value)) => write_sample(
                dw,
                &DdsKeyedString::new(key, value),
                handle,
                Some((ts_sec, ts_nanosec)),
            ),
            (Err(status), _) | (_, Err(status)) => status,
        }
    }
}

/// `BytesDataWriter::write_w_bytes`: writes `bytes[offset .. offset +
/// length]`. `handle` is 0 or the instance's.
///
/// # Safety
/// `dw` is a live writer over a `DDS::Bytes` topic; `bytes[0..bytes_len]`
/// is readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_bytes_writer_write_w_bytes(
    dw: *mut ZeroDdsDataWriter,
    bytes: *const u8,
    bytes_len: usize,
    offset: i32,
    length: i32,
    handle: u64,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe {
        match octets(bytes, bytes_len, offset, length) {
            Ok(range) => write_sample(dw, &DdsBytes::new(range), handle, None),
            Err(status) => status,
        }
    }
}

/// `BytesDataWriter::write_w_bytes_w_timestamp`.
///
/// # Safety
/// As [`zerodds_bytes_writer_write_w_bytes`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_bytes_writer_write_w_bytes_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    bytes: *const u8,
    bytes_len: usize,
    offset: i32,
    length: i32,
    handle: u64,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe {
        match octets(bytes, bytes_len, offset, length) {
            Ok(range) => write_sample(
                dw,
                &DdsBytes::new(range),
                handle,
                Some((ts_sec, ts_nanosec)),
            ),
            Err(status) => status,
        }
    }
}

/// `KeyedBytesDataWriter::write_bytes_w_key`.
///
/// # Safety
/// `dw` is a live writer over a `DDS::KeyedBytes` topic; `key` is a
/// NUL-terminated string and `bytes[0..bytes_len]` is readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_writer_write_bytes_w_key(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    bytes: *const u8,
    bytes_len: usize,
    offset: i32,
    length: i32,
    handle: u64,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe {
        match (text(key), octets(bytes, bytes_len, offset, length)) {
            (Ok(key), Ok(range)) => write_sample(dw, &DdsKeyedBytes::new(key, range), handle, None),
            (Err(status), _) | (_, Err(status)) => status,
        }
    }
}

/// `KeyedBytesDataWriter::write_bytes_w_key_w_timestamp`.
///
/// # Safety
/// As [`zerodds_keyed_bytes_writer_write_bytes_w_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_writer_write_bytes_w_key_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    bytes: *const u8,
    bytes_len: usize,
    offset: i32,
    length: i32,
    handle: u64,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe {
        match (text(key), octets(bytes, bytes_len, offset, length)) {
            (Ok(key), Ok(range)) => write_sample(
                dw,
                &DdsKeyedBytes::new(key, range),
                handle,
                Some((ts_sec, ts_nanosec)),
            ),
            (Err(status), _) | (_, Err(status)) => status,
        }
    }
}

// ---------------------------------------------------------------------------
// The instance operations of the two keyed writers
// ---------------------------------------------------------------------------

/// A keyed built-in type, with a key and no payload.
trait Keyed: DdsType {
    fn with_key(key: &str) -> Self;
    fn key_text(&self) -> &str;
}

impl Keyed for DdsKeyedString {
    fn with_key(key: &str) -> Self {
        Self::new(key, "")
    }
    fn key_text(&self) -> &str {
        &self.key
    }
}

impl Keyed for DdsKeyedBytes {
    fn with_key(key: &str) -> Self {
        Self::new(key, Vec::new())
    }
    fn key_text(&self) -> &str {
        &self.key
    }
}

/// # Safety
/// `dw` is live and over the keyed type `T`; `key` is NUL-terminated;
/// `out_handle` is NULL or writable.
unsafe fn register_w_key<T: Keyed>(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    out_handle: *mut u64,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: the caller's contract.
    let key = match unsafe { text(key) } {
        Ok(key) => key,
        Err(status) => return status,
    };
    // SAFETY: `dw` is live.
    match unsafe { register_with_holder(dw, &T::with_key(key)) } {
        Ok(handle) => {
            if !out_handle.is_null() {
                // SAFETY: writable per the contract.
                unsafe { *out_handle = handle };
            }
            ok()
        }
        Err(status) => status,
    }
}

/// # Safety
/// As [`register_w_key`], with the timestamp checked by the caller.
unsafe fn unregister_w_key<T: Keyed>(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    stamp: Option<(i32, u32)>,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: the caller's contract.
    let key = match unsafe { text(key) } {
        Ok(key) => key,
        Err(status) => return status,
    };
    // SAFETY: `dw` is live.
    let handle = unsafe { handle_of(dw, &T::with_key(key)) };
    if handle == 0 {
        return bad_parameter();
    }
    // SAFETY: `dw` is live.
    unsafe {
        match stamp {
            None => crate::extra_ffi::zerodds_dw_unregister_instance(dw, handle),
            Some((sec, nanosec)) => crate::extra_ffi::zerodds_dw_unregister_instance_w_timestamp(
                dw, handle, sec, nanosec,
            ),
        }
    }
}

/// # Safety
/// As [`register_w_key`].
unsafe fn dispose_w_key<T: Keyed>(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    stamp: Option<(i32, u32)>,
) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: the caller's contract.
    let key = match unsafe { text(key) } {
        Ok(key) => key,
        Err(status) => return status,
    };
    let Some(hash) = T::with_key(key).compute_key_hash() else {
        return bad_parameter();
    };
    // SAFETY: `dw` is live and `hash` is 16 octets.
    unsafe {
        match stamp {
            None => crate::publisher_ffi::zerodds_dw_dispose(dw, hash.as_ptr(), 0),
            Some((sec, nanosec)) => {
                crate::extra_ffi::zerodds_dw_dispose_w_timestamp(dw, hash.as_ptr(), 0, sec, nanosec)
            }
        }
    }
}

/// # Safety
/// As [`register_w_key`].
unsafe fn lookup_w_key<T: Keyed>(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    out_handle: *mut u64,
) -> c_int {
    if dw.is_null() || out_handle.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: the caller's contract.
    let key = match unsafe { text(key) } {
        Ok(key) => key,
        Err(status) => return status,
    };
    // SAFETY: `dw` is live and `out_handle` writable.
    unsafe { *out_handle = handle_of(dw, &T::with_key(key)) };
    ok()
}

/// # Safety
/// As [`register_w_key`]; `out_key` is writable.
unsafe fn get_key_value_w_key<T: Keyed>(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    out_key: *mut *mut c_char,
) -> c_int {
    if dw.is_null() || out_key.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: `dw` is live.
    let tracker = unsafe { &(*dw).instances };
    let instance = zerodds_dcps::instance_handle::InstanceHandle::from_raw(handle);
    if tracker.key_hash_for_handle(instance).is_none() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    let Some(holder) = tracker.get_key_holder(instance) else {
        return ZeroDdsStatus::Unsupported as c_int;
    };
    let Ok(sample) = T::decode_key_holder_be(&holder) else {
        return ZeroDdsStatus::Error as c_int;
    };
    match give_string(sample.key_text()) {
        Ok(key) => {
            // SAFETY: writable per the contract.
            unsafe { *out_key = key };
            ok()
        }
        Err(status) => status,
    }
}

/// Copies `ts_nanosec` into the range the DDS time takes; a value of 10^9
/// or more is not a time (DDS 1.4 §2.3.2).
fn stamp(ts_sec: i32, ts_nanosec: u32) -> Result<Option<(i32, u32)>, c_int> {
    if ts_nanosec >= 1_000_000_000 {
        Err(bad_parameter())
    } else {
        Ok(Some((ts_sec, ts_nanosec)))
    }
}

/// `KeyedStringDataWriter::register_instance_w_key`: the handle of the key's instance, registering it when it is new.
///
/// # Safety
/// `dw` is a live writer over the type; `key` is NUL-terminated; `out_handle` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_writer_register_instance_w_key(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    out_handle: *mut u64,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { register_w_key::<DdsKeyedString>(dw, key, out_handle) }
}

/// `KeyedStringDataWriter::register_instance_w_key_w_timestamp`. A registration sends nothing, so the timestamp is only checked.
///
/// # Safety
/// As the function without a timestamp.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_writer_register_instance_w_key_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    ts_sec: i32,
    ts_nanosec: u32,
    out_handle: *mut u64,
) -> c_int {
    if let Err(status) = stamp(ts_sec, ts_nanosec) {
        return status;
    }
    // SAFETY: the caller's contract.
    unsafe { register_w_key::<DdsKeyedString>(dw, key, out_handle) }
}

/// `KeyedStringDataWriter::unregister_instance_w_key`.
///
/// # Safety
/// `dw` is a live writer over the type; `key` is NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_writer_unregister_instance_w_key(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { unregister_w_key::<DdsKeyedString>(dw, key, None) }
}

/// `KeyedStringDataWriter::unregister_instance_w_key_w_timestamp`.
///
/// # Safety
/// As the function without a timestamp.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_writer_unregister_instance_w_key_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    let stamp = match stamp(ts_sec, ts_nanosec) {
        Ok(stamp) => stamp,
        Err(status) => return status,
    };
    // SAFETY: the caller's contract.
    unsafe { unregister_w_key::<DdsKeyedString>(dw, key, stamp) }
}

/// `KeyedStringDataWriter::dispose_w_key`.
///
/// # Safety
/// `dw` is a live writer over the type; `key` is NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_writer_dispose_w_key(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { dispose_w_key::<DdsKeyedString>(dw, key, None) }
}

/// `KeyedStringDataWriter::dispose_w_key_w_timestamp`.
///
/// # Safety
/// As the function without a timestamp.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_writer_dispose_w_key_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    let stamp = match stamp(ts_sec, ts_nanosec) {
        Ok(stamp) => stamp,
        Err(status) => return status,
    };
    // SAFETY: the caller's contract.
    unsafe { dispose_w_key::<DdsKeyedString>(dw, key, stamp) }
}

/// `KeyedStringDataWriter::lookup_instance_w_key`: the handle, 0 for a key that is not registered.
///
/// # Safety
/// `dw` is a live writer over the type; `key` is NUL-terminated; `out_handle` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_writer_lookup_instance_w_key(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    out_handle: *mut u64,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { lookup_w_key::<DdsKeyedString>(dw, key, out_handle) }
}

/// `KeyedStringDataWriter::get_key_value_w_key`: the key of the instance, released with `zerodds_builtin_string_free`.
///
/// # Safety
/// `dw` is a live writer over the type; `out_key` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_writer_get_key_value_w_key(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    out_key: *mut *mut c_char,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { get_key_value_w_key::<DdsKeyedString>(dw, handle, out_key) }
}

/// `KeyedBytesDataWriter::register_instance_w_key`: the handle of the key's instance, registering it when it is new.
///
/// # Safety
/// `dw` is a live writer over the type; `key` is NUL-terminated; `out_handle` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_writer_register_instance_w_key(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    out_handle: *mut u64,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { register_w_key::<DdsKeyedBytes>(dw, key, out_handle) }
}

/// `KeyedBytesDataWriter::register_instance_w_key_w_timestamp`. A registration sends nothing, so the timestamp is only checked.
///
/// # Safety
/// As the function without a timestamp.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_writer_register_instance_w_key_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    ts_sec: i32,
    ts_nanosec: u32,
    out_handle: *mut u64,
) -> c_int {
    if let Err(status) = stamp(ts_sec, ts_nanosec) {
        return status;
    }
    // SAFETY: the caller's contract.
    unsafe { register_w_key::<DdsKeyedBytes>(dw, key, out_handle) }
}

/// `KeyedBytesDataWriter::unregister_instance_w_key`.
///
/// # Safety
/// `dw` is a live writer over the type; `key` is NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_writer_unregister_instance_w_key(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { unregister_w_key::<DdsKeyedBytes>(dw, key, None) }
}

/// `KeyedBytesDataWriter::unregister_instance_w_key_w_timestamp`.
///
/// # Safety
/// As the function without a timestamp.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_writer_unregister_instance_w_key_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    let stamp = match stamp(ts_sec, ts_nanosec) {
        Ok(stamp) => stamp,
        Err(status) => return status,
    };
    // SAFETY: the caller's contract.
    unsafe { unregister_w_key::<DdsKeyedBytes>(dw, key, stamp) }
}

/// `KeyedBytesDataWriter::dispose_w_key`.
///
/// # Safety
/// `dw` is a live writer over the type; `key` is NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_writer_dispose_w_key(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { dispose_w_key::<DdsKeyedBytes>(dw, key, None) }
}

/// `KeyedBytesDataWriter::dispose_w_key_w_timestamp`.
///
/// # Safety
/// As the function without a timestamp.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_writer_dispose_w_key_w_timestamp(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    ts_sec: i32,
    ts_nanosec: u32,
) -> c_int {
    let stamp = match stamp(ts_sec, ts_nanosec) {
        Ok(stamp) => stamp,
        Err(status) => return status,
    };
    // SAFETY: the caller's contract.
    unsafe { dispose_w_key::<DdsKeyedBytes>(dw, key, stamp) }
}

/// `KeyedBytesDataWriter::lookup_instance_w_key`: the handle, 0 for a key that is not registered.
///
/// # Safety
/// `dw` is a live writer over the type; `key` is NUL-terminated; `out_handle` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_writer_lookup_instance_w_key(
    dw: *mut ZeroDdsDataWriter,
    key: *const c_char,
    out_handle: *mut u64,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { lookup_w_key::<DdsKeyedBytes>(dw, key, out_handle) }
}

/// `KeyedBytesDataWriter::get_key_value_w_key`: the key of the instance, released with `zerodds_builtin_string_free`.
///
/// # Safety
/// `dw` is a live writer over the type; `out_key` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_writer_get_key_value_w_key(
    dw: *mut ZeroDdsDataWriter,
    handle: u64,
    out_key: *mut *mut c_char,
) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { get_key_value_w_key::<DdsKeyedBytes>(dw, handle, out_key) }
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

/// One sample, taken or read, decoded as `T`. `None` as the value for a
/// sample that carries no data (a dispose or unregister marker); the info
/// is filled either way.
///
/// # Safety
/// `dr` is a live reader over `T`; `info` is writable.
unsafe fn next_sample<T: DdsType>(
    dr: *mut ZeroDdsDataReader,
    destructive: bool,
    info: *mut ZeroDdsSampleInfo,
) -> Result<Option<T>, c_int> {
    if dr.is_null() || info.is_null() {
        return Err(bad_parameter());
    }
    let mut array = ZeroDdsSampleArray {
        buffers: ptr::null_mut(),
        lengths: ptr::null_mut(),
        infos: ptr::null_mut(),
        count: 0,
        loan_token: ptr::null_mut(),
    };
    // SAFETY: `dr` is live and `array` is a valid out-parameter.
    let status = unsafe {
        if destructive {
            zerodds_dr_take(dr, &raw mut array, 1, 0, 0, 0)
        } else {
            zerodds_dr_read(dr, &raw mut array, 1, 0, 0, 0)
        }
    };
    if status != ok() {
        return Err(status);
    }
    if array.count == 0 {
        return Err(ZeroDdsStatus::NoData as c_int);
    }
    // SAFETY: the take returned one sample, so index 0 of the three arrays
    // is initialised; they stay valid until the loan is returned.
    let (payload, sample_info) = unsafe {
        let length = *array.lengths;
        let buffer = *array.buffers;
        let payload: Vec<u8> = if length == 0 || buffer.is_null() {
            Vec::new()
        } else {
            slice::from_raw_parts(buffer, length).to_vec()
        };
        (payload, *array.infos)
    };
    // SAFETY: the loan came from the call above; `info` is writable.
    unsafe {
        *info = sample_info;
        zerodds_dr_return_loan(dr, &raw mut array);
    }
    if !sample_info.valid_data {
        return Ok(None);
    }
    let big = sample_info.big_endian != 0;
    let decoded = match (sample_info.representation, big) {
        (0, false) => T::decode_xcdr1(&payload),
        (0, true) => T::decode_xcdr1_be(&payload),
        (_, false) => T::decode(&payload),
        (_, true) => T::decode_be(&payload),
    };
    decoded.map(Some).map_err(|_| ZeroDdsStatus::Error as c_int)
}

/// Hands a decoded string out; NULL for a marker.
fn put_string(slot: *mut *mut c_char, value: Option<&str>) -> c_int {
    if slot.is_null() {
        return bad_parameter();
    }
    let given = match value {
        Some(value) => match give_string(value) {
            Ok(given) => given,
            Err(status) => return status,
        },
        None => ptr::null_mut(),
    };
    // SAFETY: non-NULL and writable per the callers' contracts.
    unsafe { *slot = given };
    ok()
}

fn put_bytes(slot: *mut *mut u8, length: *mut usize, value: Option<&[u8]>) -> c_int {
    if slot.is_null() || length.is_null() {
        return bad_parameter();
    }
    let (given, len) = value.map_or((ptr::null_mut(), 0), give_bytes);
    // SAFETY: non-NULL and writable per the callers' contracts.
    unsafe {
        *slot = given;
        *length = len;
    }
    ok()
}

/// `StringDataReader::take` for one sample: `*out_value` is the string,
/// released with `zerodds_builtin_string_free`, or NULL for a sample with
/// no data (see `out_info->valid_data`).
///
/// `NoData` when the reader holds none.
///
/// # Safety
/// `dr` is a live reader over a `DDS::String` topic; the out-pointers are
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_string_reader_take_next(
    dr: *mut ZeroDdsDataReader,
    out_value: *mut *mut c_char,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    // SAFETY: the caller's contract.
    match unsafe { next_sample::<DdsString>(dr, true, out_info) } {
        Ok(sample) => put_string(out_value, sample.as_ref().map(|s| s.value.as_str())),
        Err(status) => status,
    }
}

/// `StringDataReader::read` for one sample, as
/// [`zerodds_string_reader_take_next`] without removing it.
///
/// # Safety
/// As [`zerodds_string_reader_take_next`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_string_reader_read_next(
    dr: *mut ZeroDdsDataReader,
    out_value: *mut *mut c_char,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    // SAFETY: the caller's contract.
    match unsafe { next_sample::<DdsString>(dr, false, out_info) } {
        Ok(sample) => put_string(out_value, sample.as_ref().map(|s| s.value.as_str())),
        Err(status) => status,
    }
}

/// `KeyedStringDataReader::take` for one sample: the key and the string,
/// both released with `zerodds_builtin_string_free`. A sample with no data
/// (a dispose marker, for instance) carries its key and no string.
///
/// # Safety
/// `dr` is a live reader over a `DDS::KeyedString` topic; the out-pointers
/// are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_reader_take_next(
    dr: *mut ZeroDdsDataReader,
    out_key: *mut *mut c_char,
    out_value: *mut *mut c_char,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    keyed_string_next(
        // SAFETY: the caller's contract.
        unsafe { next_sample::<DdsKeyedString>(dr, true, out_info) },
        out_key,
        out_value,
    )
}

/// `KeyedStringDataReader::read` for one sample.
///
/// # Safety
/// As [`zerodds_keyed_string_reader_take_next`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_string_reader_read_next(
    dr: *mut ZeroDdsDataReader,
    out_key: *mut *mut c_char,
    out_value: *mut *mut c_char,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    keyed_string_next(
        // SAFETY: the caller's contract.
        unsafe { next_sample::<DdsKeyedString>(dr, false, out_info) },
        out_key,
        out_value,
    )
}

fn keyed_string_next(
    sample: Result<Option<DdsKeyedString>, c_int>,
    out_key: *mut *mut c_char,
    out_value: *mut *mut c_char,
) -> c_int {
    match sample {
        Ok(sample) => {
            let status = put_string(out_key, sample.as_ref().map(|s| s.key.as_str()));
            if status != ok() {
                return status;
            }
            put_string(out_value, sample.as_ref().map(|s| s.value.as_str()))
        }
        Err(status) => status,
    }
}

/// `BytesDataReader::take` for one sample: the bytes, released with
/// `zerodds_builtin_bytes_free`; NULL and length 0 for a sample with no data
/// and for empty bytes (`out_info->valid_data` tells them apart).
///
/// # Safety
/// `dr` is a live reader over a `DDS::Bytes` topic; the out-pointers are
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_bytes_reader_take_next(
    dr: *mut ZeroDdsDataReader,
    out_bytes: *mut *mut u8,
    out_len: *mut usize,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    // SAFETY: the caller's contract.
    match unsafe { next_sample::<DdsBytes>(dr, true, out_info) } {
        Ok(sample) => put_bytes(
            out_bytes,
            out_len,
            sample.as_ref().map(|s| s.value.as_slice()),
        ),
        Err(status) => status,
    }
}

/// `BytesDataReader::read` for one sample.
///
/// # Safety
/// As [`zerodds_bytes_reader_take_next`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_bytes_reader_read_next(
    dr: *mut ZeroDdsDataReader,
    out_bytes: *mut *mut u8,
    out_len: *mut usize,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    // SAFETY: the caller's contract.
    match unsafe { next_sample::<DdsBytes>(dr, false, out_info) } {
        Ok(sample) => put_bytes(
            out_bytes,
            out_len,
            sample.as_ref().map(|s| s.value.as_slice()),
        ),
        Err(status) => status,
    }
}

/// `KeyedBytesDataReader::take` for one sample: the key and the bytes.
///
/// # Safety
/// `dr` is a live reader over a `DDS::KeyedBytes` topic; the out-pointers
/// are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_reader_take_next(
    dr: *mut ZeroDdsDataReader,
    out_key: *mut *mut c_char,
    out_bytes: *mut *mut u8,
    out_len: *mut usize,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    keyed_bytes_next(
        // SAFETY: the caller's contract.
        unsafe { next_sample::<DdsKeyedBytes>(dr, true, out_info) },
        out_key,
        out_bytes,
        out_len,
    )
}

/// `KeyedBytesDataReader::read` for one sample.
///
/// # Safety
/// As [`zerodds_keyed_bytes_reader_take_next`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_keyed_bytes_reader_read_next(
    dr: *mut ZeroDdsDataReader,
    out_key: *mut *mut c_char,
    out_bytes: *mut *mut u8,
    out_len: *mut usize,
    out_info: *mut ZeroDdsSampleInfo,
) -> c_int {
    keyed_bytes_next(
        // SAFETY: the caller's contract.
        unsafe { next_sample::<DdsKeyedBytes>(dr, false, out_info) },
        out_key,
        out_bytes,
        out_len,
    )
}

fn keyed_bytes_next(
    sample: Result<Option<DdsKeyedBytes>, c_int>,
    out_key: *mut *mut c_char,
    out_bytes_slot: *mut *mut u8,
    out_len: *mut usize,
) -> c_int {
    match sample {
        Ok(sample) => {
            let status = put_string(out_key, sample.as_ref().map(|s| s.key.as_str()));
            if status != ok() {
                return status;
            }
            put_bytes(
                out_bytes_slot,
                out_len,
                sample.as_ref().map(|s| s.value.as_slice()),
            )
        }
        Err(status) => status,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// A string a function of this module hands out is the one that was
    /// given, and is released.
    #[test]
    fn strings_and_arrays_go_out_and_are_released() {
        let given = give_string("hello").unwrap();
        // SAFETY: from `give_string`.
        unsafe {
            assert_eq!(CStr::from_ptr(given).to_str().unwrap(), "hello");
            zerodds_builtin_string_free(given);
            zerodds_builtin_string_free(ptr::null_mut());
        }
        assert!(
            give_string("a\0b").is_err(),
            "an interior NUL is no C string"
        );
        let (bytes, len) = give_bytes(&[1, 2, 3]);
        // SAFETY: from `give_bytes`.
        unsafe {
            assert_eq!(slice::from_raw_parts(bytes, len), [1, 2, 3]);
            zerodds_builtin_bytes_free(bytes, len);
        }
        assert_eq!(give_bytes(&[]), (ptr::null_mut(), 0));
    }

    /// Annex E `write_w_bytes`: the octets `offset .. offset + length`, and
    /// a range outside the array is refused.
    #[test]
    fn a_byte_range_is_checked() {
        let data = [10_u8, 20, 30, 40];
        // SAFETY: a live array.
        unsafe {
            assert_eq!(octets(data.as_ptr(), 4, 1, 2).unwrap(), [20, 30]);
            assert_eq!(octets(data.as_ptr(), 4, 4, 0).unwrap(), [] as [u8; 0]);
            assert!(octets(data.as_ptr(), 4, 3, 2).is_err());
            assert!(octets(data.as_ptr(), 4, -1, 1).is_err());
            assert!(octets(data.as_ptr(), 4, 0, -1).is_err());
            assert!(octets(ptr::null(), 3, 0, 1).is_err());
            assert_eq!(octets(ptr::null(), 0, 0, 0).unwrap(), [] as [u8; 0]);
        }
    }

    /// The kinds name the four physical types, and each has a
    /// `TypeObject` the writer and reader advertise.
    #[test]
    fn the_kinds_name_the_four_types() {
        let names = [
            (ZERODDS_BUILTIN_STRING, "DDS::String", false),
            (ZERODDS_BUILTIN_KEYED_STRING, "DDS::KeyedString", true),
            (ZERODDS_BUILTIN_BYTES, "DDS::Bytes", false),
            (ZERODDS_BUILTIN_KEYED_BYTES, "DDS::KeyedBytes", true),
        ];
        for (kind, name, keyed) in names {
            assert_eq!(type_name_of(kind), Some(name));
            let (object, is_keyed) = type_object_of(name).expect("a TypeObject");
            assert!(!object.is_empty());
            assert_eq!(is_keyed, keyed);
        }
        assert_eq!(type_name_of(4), None);
        assert!(type_object_of("DDS::Other").is_none());
    }

    /// A handle that is not the key's instance is refused on a keyed
    /// write; the key hash of a keyed sample is its key's, whatever the
    /// payload.
    #[test]
    fn the_key_of_a_sample_is_its_key() {
        let a = DdsKeyedString::new("k", "one").compute_key_hash();
        let b = DdsKeyedString::new("k", "two").compute_key_hash();
        let c = DdsKeyedString::new("other", "one").compute_key_hash();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(DdsString::new("x").compute_key_hash(), None);
    }

    /// An invalid timestamp is refused where the call sends nothing.
    #[test]
    fn a_nanosecond_field_of_a_second_is_no_time() {
        assert!(stamp(1, 1_000_000_000).is_err());
        assert_eq!(stamp(1, 5).unwrap(), Some((1, 5)));
    }
}
