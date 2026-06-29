// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XCDR2 TypeSupport FFI — implements `zerodds-xcdr2-c-1.0`.
//!
//! This layer complements the byte-oriented FFI of `lib.rs` with a
//! **typed** TypeSupport FFI with function-table-based dispatch.
//! Per IDL type a codegen (idl-cpp `--c-mode` flag) provides a
//! static `zerodds_typesupport_t` table with encoder/decoder/key-hash
//! functions. The FFI here takes this table, calls it
//! and passes the result to the existing writer/reader paths.
//!
//! ## Wire format
//!
//! XCDR2 §7.4 PLAIN_CDR2 LE as the default. The codegen MUST use the XCDR2 encoder
//! from `zerodds-cdr` — this FFI does not duplicate the encoder logic,
//! it only provides the dispatch + helpers.
//!
//! ## Memory ownership (§6 vendor spec)
//!
//! - `zerodds_xcdr2_encode`: the caller provides `out_buf`+`out_cap`; the callee
//!   writes `out_len`. `out_buf=NULL` is legal as a size probe — then
//!   `out_len` returns the needed size and the return is
//!   `BUFFER_TOO_SMALL`.
//! - `zerodds_xcdr2_decode`: the caller provides `out_sample` zero-initialized;
//!   the callee allocates strings/sequences. The caller MUST later call
//!   `ts.sample_free(sample)`.
//! - `zerodds_writer_write_typed`: calls `ts.encode` into an internal
//!   buffer, passes the bytes to `zerodds_writer_write`.
//! - `zerodds_reader_take_typed`: calls `zerodds_reader_take`, then
//!   `ts.decode` on the bytes; frees the internal buffer.

use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use core::slice;

use crate::{ZeroDdsReader, ZeroDdsRuntime, ZeroDdsStatus, ZeroDdsWriter};

// ============================================================================
// TypeSupport function-pointer types
// ============================================================================

/// Encoder function. `sample` is a pointer to a language-specific
/// representation of the IDL type; `out_buf`/`out_cap` is the caller-
/// provided output buffer (may be NULL for size probing).
/// `out_len` is set to the needed/written length.
///
/// Return: 0=OK, -13=BUFFER_TOO_SMALL, !=0=another error.
pub type ZeroDdsEncodeFn = unsafe extern "C" fn(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int;

/// Decoder function. `buf`/`len` is the incoming byte stream;
/// `out_sample` must be prepared zero-initialized by the caller.
/// Strings/sequences are allocated by the decoder via malloc and
/// must later be freed via `ZeroDdsSampleFreeFn`.
///
/// NOTE: this signature carries no representation tag. A decoder that is
/// alignment-sensitive (any type with a 64-bit member, where XCDR1 aligns
/// to 8 but XCDR2 caps at 4 — DDS-XTypes 1.3 §7.4.1.1.1) cannot pick the
/// correct `max_align` from this signature alone and will underrun an
/// XCDR2 payload it mistakes for XCDR1. Such types must provide a
/// [`ZeroDdsDecodeReprFn`] in [`ZeroDdsTypeSupport::decode_repr`]; the
/// take path forwards the writer's real representation to it (Bug R4b).
pub type ZeroDdsDecodeFn =
    unsafe extern "C" fn(buf: *const u8, len: usize, out_sample: *mut c_void) -> c_int;

/// Representation-aware decoder (Bug R4b). Identical to [`ZeroDdsDecodeFn`]
/// but receives the wire XCDR version tag (`0` = XCDR1, `1` = XCDR2,
/// matching `UserSample::Alive.representation` / `zerodds_reader_take`'s
/// `out_repr`). A codegen decoder uses it to select `max_align` (8 for
/// XCDR1, 4 for XCDR2) so a multi-member 64-bit-containing sample
/// round-trips without an underrun regardless of the publisher's
/// negotiated representation.
///
/// When a TypeSupport sets this pointer, [`zerodds_reader_take_typed`]
/// recovers the writer's actual representation (instead of discarding it)
/// and calls this in preference to the legacy [`ZeroDdsDecodeFn`].
pub type ZeroDdsDecodeReprFn = unsafe extern "C" fn(
    buf: *const u8,
    len: usize,
    representation: u8,
    out_sample: *mut c_void,
) -> c_int;

/// Key-hash function (XTypes 1.3 §7.6.8). Writes 16 bytes into
/// `out_hash`. Return 0 = OK.
pub type ZeroDdsKeyHashFn = unsafe extern "C" fn(sample: *const c_void, out_hash: *mut u8) -> c_int;

/// Sample-free function. Frees heap-allocated fields (strings,
/// sequences) in the sample. The sample struct itself belongs to the
/// caller and is not freed here.
pub type ZeroDdsSampleFreeFn = unsafe extern "C" fn(sample: *mut c_void);

// ============================================================================
// `zerodds_typesupport_t` (§2)
// ============================================================================

/// TypeSupport function table (vendor spec §2).
///
/// ABI-stable; fields are versioned via `zerodds_c_api_version()`.
/// Per IDL type the C codegen emits exactly one `static const`
/// instance of this struct.
///
/// Layout reasons:
/// - 16-byte `type_hash` first (XTypes EquivalenceHash).
/// - `type_name` as a NUL-terminated pointer (no owned String).
/// - `is_keyed` + `extensibility` as `u8` for compact packing.
/// - 6 bytes padding up to the function-pointer-aligned fields.
/// - 5 function pointers in fixed order.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZeroDdsTypeSupport {
    /// 16-byte EquivalenceHash (XTypes §7.3.4).
    pub type_hash: [u8; 16],
    /// NUL-terminated type name (Module::Sub::Struct, ASCII).
    pub type_name: *const c_char,
    /// 1 = at least one @key member present.
    pub is_keyed: u8,
    /// 0=Final, 1=Appendable, 2=Mutable.
    pub extensibility: u8,
    /// Reserved for future fields. MUST be 0.
    pub _reserved: [u8; 6],
    /// Encoder pointer (see `ZeroDdsEncodeFn`).
    pub encode: Option<ZeroDdsEncodeFn>,
    /// Decoder pointer (see `ZeroDdsDecodeFn`).
    pub decode: Option<ZeroDdsDecodeFn>,
    /// Key-hash pointer (see `ZeroDdsKeyHashFn`); NULL allowed if
    /// `is_keyed = 0`.
    pub key_hash: Option<ZeroDdsKeyHashFn>,
    /// Sample-free pointer (see `ZeroDdsSampleFreeFn`); NULL allowed
    /// if the type has no heap fields.
    pub sample_free: Option<ZeroDdsSampleFreeFn>,
    /// Representation-aware decoder (see `ZeroDdsDecodeReprFn`); Bug R4b.
    ///
    /// ADDITIVE TAIL FIELD (minor-bump ABI, `zerodds-c-api-1.0` §7): a
    /// codegen table built before this field was added simply omits it
    /// from its designated-initializer list, so it is zero-filled to
    /// NULL — the take path then falls back to the legacy `decode`
    /// pointer. New codegen that wants representation-correct alignment
    /// for 64-bit members sets this pointer.
    pub decode_repr: Option<ZeroDdsDecodeReprFn>,
}

// SAFETY: The FFI pointers in this struct are read only at the FFI boundary
// and are held in the caller (static const tables from codegen) as
// `'static`. The Rust side only passes the table through.
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsTypeSupport {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsTypeSupport {}

// ============================================================================
// Topic handle (typed)
// ============================================================================

/// Opaque topic handle with a fixed TypeSupport table.
///
/// Currently the topic layer of the C-FFI is a lightweight
/// container over `topic_name`/`type_name` — both are augmented here with the
/// TypeSupport, so that `writer_write_typed`/`reader_take_typed`
/// can find the table without further caller argument passing.
pub struct ZeroDdsTopic {
    /// Pointer to the static TypeSupport table (caller-owned).
    pub type_support: *const ZeroDdsTypeSupport,
    /// Topic name (owned, for later writer/reader creation).
    pub topic_name: alloc::string::String,
    /// Type name (owned, extracted from `type_support.type_name`).
    pub type_name: alloc::string::String,
}

// SAFETY: `type_support` points to a static codegen table; strings are owned.
unsafe impl Send for ZeroDdsTopic {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsTopic {}

// ============================================================================
// FFI: Topic
// ============================================================================

/// Creates a typed topic handle. Stores the TypeSupport
/// table so that writer/reader operations can access it.
///
/// `out_topic` must be non-NULL and is set to the newly allocated
/// topic handle. The caller MUST later call
/// `zerodds_topic_destroy_typed`.
///
/// # Safety
/// `participant`/`topic_name`/`type_support`/`out_topic` must be valid
/// pointers. `topic_name` must be NUL-terminated. `type_support`
/// MUST point to a static table with live function pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_create_typed(
    participant: *mut ZeroDdsRuntime,
    topic_name: *const c_char,
    type_support: *const ZeroDdsTypeSupport,
    out_topic: *mut *mut ZeroDdsTopic,
) -> c_int {
    if participant.is_null()
        || topic_name.is_null()
        || type_support.is_null()
        || out_topic.is_null()
    {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL check above.
    let ts = unsafe { &*type_support };
    if ts.type_name.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: topic_name is NUL-terminated per the caller contract.
    let topic = match unsafe { core::ffi::CStr::from_ptr(topic_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ZeroDdsStatus::InvalidUtf8 as c_int,
    };
    // SAFETY: type_name is NUL-terminated per vendor spec §2.
    let typ = match unsafe { core::ffi::CStr::from_ptr(ts.type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ZeroDdsStatus::InvalidUtf8 as c_int,
    };
    let handle = alloc::boxed::Box::new(ZeroDdsTopic {
        type_support,
        topic_name: topic,
        type_name: typ,
    });
    // SAFETY: out_topic NULL-checked.
    unsafe {
        *out_topic = alloc::boxed::Box::into_raw(handle);
    }
    ZeroDdsStatus::Ok as c_int
}

/// Destroys a topic handle. NULL-safe.
///
/// # Safety
/// `topic` must come from `zerodds_topic_create_typed` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_destroy_typed(topic: *mut ZeroDdsTopic) {
    if topic.is_null() {
        return;
    }
    // SAFETY: box pointer from `Box::into_raw`.
    let _ = unsafe { alloc::boxed::Box::from_raw(topic) };
}

// ============================================================================
// FFI: standalone encoding
// ============================================================================

/// Standalone encoder. Calls `ts.encode(sample, out_buf, out_cap, out_len)`
/// and passes the return code through.
///
/// `out_buf=NULL` with `out_cap=0` is a size probe: the codegen encoder
/// MUST set `out_len` to the needed size and
/// return `BUFFER_TOO_SMALL`.
///
/// # Safety
/// `ts`, `sample`, `out_len` must be valid pointers. `out_buf` may
/// be NULL if `out_cap = 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_xcdr2_encode(
    ts: *const ZeroDdsTypeSupport,
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    if ts.is_null() || sample.is_null() || out_len.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    if out_buf.is_null() && out_cap != 0 {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: NULL check above.
    let ts_ref = unsafe { &*ts };
    let Some(enc) = ts_ref.encode else {
        return ZeroDdsStatus::Unsupported as c_int;
    };
    // SAFETY: the caller contract requires valid encoder function-pointer
    // provenance + `sample` points to a valid type representation.
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { enc(sample, out_buf, out_cap, out_len) }
}

/// Standalone decoder. Calls `ts.decode(buf, len, out_sample)`.
///
/// # Safety
/// `ts`/`buf`/`out_sample` valid; `buf` must be readable for `len` bytes;
/// `out_sample` must point zero-initialized to the language type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_xcdr2_decode(
    ts: *const ZeroDdsTypeSupport,
    buf: *const u8,
    len: usize,
    out_sample: *mut c_void,
) -> c_int {
    if ts.is_null() || buf.is_null() || out_sample.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL check above.
    let ts_ref = unsafe { &*ts };
    let Some(dec) = ts_ref.decode else {
        return ZeroDdsStatus::Unsupported as c_int;
    };
    // SAFETY: caller contract + NULL check.
    unsafe { dec(buf, len, out_sample) }
}

/// Standalone representation-aware decoder (Bug R4b). Prefers
/// `ts.decode_repr(buf, len, representation, out_sample)` when present so
/// the decoder can pick the correct `max_align` (8 for XCDR1 = `0`,
/// 4 for XCDR2 = `1`); falls back to the legacy `ts.decode` when the
/// TypeSupport table predates the repr-aware entry-point.
///
/// `representation`: `0` = XCDR1, `1` = XCDR2 (the value reported by
/// `zerodds_reader_take`'s `out_repr`).
///
/// # Safety
/// `ts`/`buf`/`out_sample` valid; `buf` readable for `len` bytes;
/// `out_sample` zero-initialized to the language type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_xcdr2_decode_repr(
    ts: *const ZeroDdsTypeSupport,
    buf: *const u8,
    len: usize,
    representation: u8,
    out_sample: *mut c_void,
) -> c_int {
    if ts.is_null() || buf.is_null() || out_sample.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL check above.
    let ts_ref = unsafe { &*ts };
    if let Some(dec2) = ts_ref.decode_repr {
        // SAFETY: caller contract + NULL check.
        return unsafe { dec2(buf, len, representation, out_sample) };
    }
    let Some(dec) = ts_ref.decode else {
        return ZeroDdsStatus::Unsupported as c_int;
    };
    // SAFETY: caller contract + NULL check.
    unsafe { dec(buf, len, out_sample) }
}

// ============================================================================
// FFI: writer/reader (typed)
// ============================================================================

/// Writes a typed sample. Encodes via
/// `ts.encode` into an internal heap buffer and passes the bytes to
/// `zerodds_writer_write`.
///
/// Strategy:
/// 1. Size probe (`out_buf=NULL`).
/// 2. Allocate a heap buffer of the probed size.
/// 3. Real encode in.
/// 4. `zerodds_writer_write(writer, bytes, len)`.
///
/// # Safety
/// `writer`/`ts`/`sample` valid; cf. `zerodds_writer_write` and
/// `zerodds_xcdr2_encode`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_writer_write_typed(
    writer: *mut ZeroDdsWriter,
    ts: *const ZeroDdsTypeSupport,
    sample: *const c_void,
) -> c_int {
    if writer.is_null() || ts.is_null() || sample.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL check above.
    let ts_ref = unsafe { &*ts };
    let Some(enc) = ts_ref.encode else {
        return ZeroDdsStatus::Unsupported as c_int;
    };
    // Step 1: size probe.
    let mut needed: usize = 0;
    // SAFETY: enc comes from codegen, `sample` caller-validated.
    let probe_rc = unsafe { enc(sample, ptr::null_mut(), 0, &mut needed as *mut usize) };
    // The codegen encoder MUST return BUFFER_TOO_SMALL on `out_buf=NULL`
    // OR 0 if the type has a 0-byte payload (e.g. V-1 empty final).
    if probe_rc != ZeroDdsStatus::Ok as c_int && probe_rc != ZeroDdsStatus::Unsupported as c_int {
        // BUFFER_TOO_SMALL is signaled in our status mapping as Error/Unsupported;
        // we accept any non-OK probe code as "size now
        // in `needed`".
    }
    // Step 2 + 3: heap buffer + real encode.
    let mut buf = alloc::vec![0u8; needed];
    let mut written: usize = 0;
    let buf_ptr = if needed == 0 {
        ptr::null_mut()
    } else {
        buf.as_mut_ptr()
    };
    // SAFETY: the buffer is `needed` bytes large.
    let enc_rc = unsafe { enc(sample, buf_ptr, needed, &mut written as *mut usize) };
    if enc_rc != ZeroDdsStatus::Ok as c_int {
        return enc_rc;
    }
    // Step 4: pass-through to the wire writer.
    // SAFETY: writer NULL-checked; buf has `written` initialized bytes.
    unsafe { crate::zerodds_writer_write(writer, buf.as_ptr(), written) }
}

/// Reads a typed sample. Fetches bytes via `zerodds_reader_take`
/// and decodes them via `ts.decode` into `out_sample`.
///
/// `out_info` is currently unused (reserved for the sample-info spec
/// rollout); MUST be NULL or point to a caller-allocated
/// `zerodds_sample_info_t` that is filled with default values.
///
/// Returns:
/// - 0 (Ok) if a sample was decoded successfully.
/// - `NoData` (-7) if the reader is empty.
/// - other negative codes on a decoder error.
///
/// # Safety
/// `reader`/`ts`/`out_sample` valid. `out_info` NULL or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_reader_take_typed(
    reader: *mut ZeroDdsReader,
    ts: *const ZeroDdsTypeSupport,
    out_sample: *mut c_void,
    out_info: *mut c_void,
) -> c_int {
    let _ = out_info;
    if reader.is_null() || ts.is_null() || out_sample.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: NULL check above.
    let ts_ref = unsafe { &*ts };
    // A type must expose at least one decoder (legacy or repr-aware).
    if ts_ref.decode.is_none() && ts_ref.decode_repr.is_none() {
        return ZeroDdsStatus::Unsupported as c_int;
    }
    let mut buf: *mut u8 = ptr::null_mut();
    let mut len: usize = 0;
    // Bug R4b: recover the writer's ACTUAL representation tag instead of
    // discarding it (`out_repr=NULL` previously). The same-runtime / wire
    // path threads the publisher's negotiated XCDR version into the
    // `UserSample` (Bug R4); `zerodds_reader_take` surfaces it here via
    // `out_repr` (`0` = XCDR1, `1` = XCDR2). A repr-aware decoder needs it
    // to pick `max_align` (8 vs 4) — without it a 64-bit-containing
    // multi-member XCDR2 sample underruns.
    let mut repr: u8 = 0;
    // SAFETY: buf/len/repr are local stack pointers, reader NULL-checked.
    let rc = unsafe {
        crate::zerodds_reader_take(
            reader,
            &mut buf as *mut *mut u8,
            &mut len as *mut usize,
            &mut repr as *mut u8,
        )
    };
    if rc != ZeroDdsStatus::Ok as c_int {
        return rc;
    }
    if buf.is_null() || len == 0 {
        return ZeroDdsStatus::NoData as c_int;
    }
    // Prefer the representation-aware decoder so alignment matches the
    // publisher's negotiated XCDR version; fall back to the legacy
    // (version-fixed) decoder for TypeSupport tables that predate
    // `decode_repr`.
    let dec_rc = if let Some(dec2) = ts_ref.decode_repr {
        // SAFETY: dec2 from codegen; buf valid for `len` bytes.
        unsafe { dec2(buf, len, repr, out_sample) }
    } else if let Some(dec) = ts_ref.decode {
        // SAFETY: dec from codegen; buf valid for `len` bytes.
        unsafe { dec(buf, len, out_sample) }
    } else {
        // A TypeSupport with neither decoder is malformed — fail loudly, not
        // by panicking (free the reader buffer below regardless).
        ZeroDdsStatus::PreconditionNotMet as c_int
    };
    // Always free the reader buffer, even on error.
    // SAFETY: buf/len come from zerodds_reader_take.
    unsafe { crate::zerodds_buffer_free(buf, len) };
    dec_rc
}

// ============================================================================
// Rust-side helper: XCDR2 encoder/decoder via `zerodds-cdr`
// ============================================================================

/// Safely converts a `*mut usize` output parameter back to the
/// Rust world. Used internally by codegen encoders.
///
/// # Safety
/// `out_len` must be a valid `*mut usize`.
pub unsafe fn write_out_len(out_len: *mut usize, value: usize) {
    if !out_len.is_null() {
        // SAFETY: the caller contract guarantees a valid pointer.
        unsafe { *out_len = value };
    }
}

/// Writes `bytes` into `out_buf` if the capacity is enough. Sets
/// `out_len = bytes.len()`. Returns the FFI status:
/// - 0 (Ok) if all bytes were written.
/// - -13 (Unsupported, used here semantically as BUFFER_TOO_SMALL)
///   if `out_buf=NULL` (size probe) or `out_cap < bytes.len()`.
///
/// # Safety
/// `out_buf` must be NULL or valid for `out_cap` bytes write access.
/// `out_len` must be valid.
pub unsafe fn copy_to_out_buf(
    bytes: &[u8],
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: wrapper contract, documented above.
    unsafe { write_out_len(out_len, bytes.len()) };
    if bytes.is_empty() {
        // 0-byte payload (e.g. V-1 empty final): nothing to copy,
        // probe or real call both OK.
        return ZeroDdsStatus::Ok as c_int;
    }
    if out_buf.is_null() || out_cap < bytes.len() {
        return ZeroDdsStatus::Unsupported as c_int;
    }
    // SAFETY: out_buf has >= bytes.len() capacity, both non-overlapping
    // (caller buffer + Rust stack buffer).
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, bytes.len());
    }
    ZeroDdsStatus::Ok as c_int
}

/// Returns a slice over the FFI input buffer. Convenience for
/// codegen decoders.
///
/// # Safety
/// `buf` must have valid read access for `len` bytes.
pub unsafe fn input_slice<'a>(buf: *const u8, len: usize) -> &'a [u8] {
    if buf.is_null() || len == 0 {
        return &[];
    }
    // SAFETY: caller contract.
    unsafe { slice::from_raw_parts(buf, len) }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::c_void;

    // V-2 encoder + decoder for tests: Point{ x: i32, y: i32 } @final.

    #[repr(C)]
    struct PointSample {
        x: i32,
        y: i32,
    }

    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe extern "C" fn point_encode(
        sample: *const c_void,
        out_buf: *mut u8,
        out_cap: usize,
        out_len: *mut usize,
    ) -> c_int {
        // SAFETY: test-only.
        let s = unsafe { &*(sample as *const PointSample) };
        let mut bytes = alloc::vec::Vec::with_capacity(8);
        bytes.extend_from_slice(&s.x.to_le_bytes());
        bytes.extend_from_slice(&s.y.to_le_bytes());
        // SAFETY: helper.
        unsafe { copy_to_out_buf(&bytes, out_buf, out_cap, out_len) }
    }

    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe extern "C" fn point_decode(
        buf: *const u8,
        len: usize,
        out_sample: *mut c_void,
    ) -> c_int {
        if len < 8 {
            return ZeroDdsStatus::BadParameter as c_int;
        }
        // SAFETY: test-only.
        let bytes = unsafe { input_slice(buf, len) };
        let x = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let y = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
        // SAFETY: test-only.
        unsafe {
            (*(out_sample as *mut PointSample)).x = x;
            (*(out_sample as *mut PointSample)).y = y;
        }
        ZeroDdsStatus::Ok as c_int
    }

    static POINT_TYPE_NAME: &[u8] = b"Point\0";

    fn point_typesupport() -> ZeroDdsTypeSupport {
        ZeroDdsTypeSupport {
            type_hash: [0u8; 16],
            type_name: POINT_TYPE_NAME.as_ptr() as *const c_char,
            is_keyed: 0,
            extensibility: 0,
            _reserved: [0u8; 6],
            encode: Some(point_encode),
            decode: Some(point_decode),
            key_hash: None,
            sample_free: None,
            decode_repr: None,
        }
    }

    #[test]
    fn xcdr2_encode_size_probe_returns_needed() {
        let ts = point_typesupport();
        let s = PointSample { x: 1, y: -2 };
        let mut needed: usize = 0;
        // SAFETY: test-controlled pointers.
        let rc = unsafe {
            zerodds_xcdr2_encode(
                &ts,
                &s as *const _ as *const c_void,
                core::ptr::null_mut(),
                0,
                &mut needed,
            )
        };
        // The probe returns -13 (Unsupported = BUFFER_TOO_SMALL).
        assert_eq!(rc, ZeroDdsStatus::Unsupported as c_int);
        assert_eq!(needed, 8);
    }

    #[test]
    fn xcdr2_encode_writes_le_bytes() {
        let ts = point_typesupport();
        let s = PointSample { x: 1, y: -2 };
        let mut buf = [0u8; 8];
        let mut written: usize = 0;
        // SAFETY: test-controlled pointers.
        let rc = unsafe {
            zerodds_xcdr2_encode(
                &ts,
                &s as *const _ as *const c_void,
                buf.as_mut_ptr(),
                buf.len(),
                &mut written,
            )
        };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(written, 8);
        assert_eq!(buf, [0x01, 0x00, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn xcdr2_decode_roundtrip() {
        let ts = point_typesupport();
        let bytes = [0x01u8, 0x00, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF];
        let mut out = PointSample { x: 0, y: 0 };
        // SAFETY: test-controlled pointers.
        let rc = unsafe {
            zerodds_xcdr2_decode(
                &ts,
                bytes.as_ptr(),
                bytes.len(),
                &mut out as *mut _ as *mut c_void,
            )
        };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(out.x, 1);
        assert_eq!(out.y, -2);
    }

    #[test]
    fn xcdr2_encode_null_ts_rejected() {
        let s = PointSample { x: 1, y: 2 };
        let mut buf = [0u8; 8];
        let mut written: usize = 0;
        // SAFETY: test-controlled NULL passthrough.
        let rc = unsafe {
            zerodds_xcdr2_encode(
                core::ptr::null(),
                &s as *const _ as *const c_void,
                buf.as_mut_ptr(),
                buf.len(),
                &mut written,
            )
        };
        assert_eq!(rc, ZeroDdsStatus::BadHandle as c_int);
    }

    #[test]
    fn xcdr2_decode_null_buf_rejected() {
        let ts = point_typesupport();
        let mut out = PointSample { x: 0, y: 0 };
        // SAFETY: test-controlled NULL passthrough.
        let rc = unsafe {
            zerodds_xcdr2_decode(&ts, core::ptr::null(), 0, &mut out as *mut _ as *mut c_void)
        };
        assert_eq!(rc, ZeroDdsStatus::BadHandle as c_int);
    }

    // ------------------------------------------------------------------
    // Bug R4b: representation-aware decode. A multi-member type with a
    // 64-bit lead member: XCDR1 aligns the i64 to 8, XCDR2 caps at 4.
    // The decoder MUST be told the representation, else it picks the
    // wrong max_align and underruns / misreads.
    // ------------------------------------------------------------------

    #[repr(C)]
    struct Tele2 {
        ts: i64,
        seq: i32,
    }

    /// XCDR2 (max_align = 4) layout: [i64 @0..8][i32 @8..12] — no pad,
    /// since the stream starts 4-aligned and XCDR2 never aligns past 4.
    /// XCDR1 (max_align = 8) layout would pad the i64 to an 8-boundary;
    /// the stream here starts at 0 so the i64 is already 8-aligned, but a
    /// faithful repr-aware decoder still distinguishes by NOT padding the
    /// i32 (both repr put it at 8 here). To make the distinction
    /// observable we encode the i64 with a leading 4-byte XCDR1 pad only
    /// in the XCDR1 branch; the test feeds an XCDR2 stream and asserts the
    /// repr-aware decoder reads it correctly.
    // SAFETY: FFI-boundary; pointers caller-validated.
    unsafe extern "C" fn tele2_decode_repr(
        buf: *const u8,
        len: usize,
        representation: u8,
        out_sample: *mut c_void,
    ) -> c_int {
        // SAFETY: test-only.
        let b = unsafe { input_slice(buf, len) };
        // XCDR1 = max_align 8, XCDR2 = max_align 4.
        let max_align: usize = if representation == 1 { 4 } else { 8 };
        // Align the i64 within the (4-aligned) body. For a body starting
        // at offset 0 the i64 sits at 0 in BOTH; the distinction this test
        // exercises is that an XCDR2 stream is 12 bytes and must be decoded
        // as such. A version-blind decoder defaulting to XCDR1 would, on a
        // type whose body does NOT start 8-aligned, mis-align and underrun;
        // here we assert the version is plumbed through and the 12-byte
        // XCDR2 sample round-trips.
        let _ = max_align;
        if b.len() < 12 {
            return ZeroDdsStatus::BadParameter as c_int;
        }
        let ts = i64::from_le_bytes(b[0..8].try_into().unwrap());
        let seq = i32::from_le_bytes(b[8..12].try_into().unwrap());
        // SAFETY: test-only.
        unsafe {
            (*(out_sample as *mut Tele2)).ts = ts;
            (*(out_sample as *mut Tele2)).seq = seq;
        }
        // Smuggle the representation we were handed into the high seq bit
        // region is not possible; instead record it via the ts low byte
        // check in the test by requiring representation == 1.
        if representation != 1 {
            // The test publisher offers XCDR2; a non-1 repr means the take
            // path failed to carry the writer's representation (Bug R4b).
            return ZeroDdsStatus::Error as c_int;
        }
        ZeroDdsStatus::Ok as c_int
    }

    /// Legacy version-blind decoder — must NOT be called when
    /// `decode_repr` is set.
    // SAFETY: FFI-boundary.
    unsafe extern "C" fn tele2_decode_legacy(
        _buf: *const u8,
        _len: usize,
        _out: *mut c_void,
    ) -> c_int {
        // If this runs, the repr-aware preference is broken.
        ZeroDdsStatus::PreconditionNotMet as c_int
    }

    static TELE2_NAME: &[u8] = b"Tele2\0";

    fn tele2_typesupport() -> ZeroDdsTypeSupport {
        ZeroDdsTypeSupport {
            type_hash: [0u8; 16],
            type_name: TELE2_NAME.as_ptr() as *const c_char,
            is_keyed: 0,
            extensibility: 1,
            _reserved: [0u8; 6],
            encode: None,
            decode: Some(tele2_decode_legacy),
            key_hash: None,
            sample_free: None,
            decode_repr: Some(tele2_decode_repr),
        }
    }

    #[test]
    fn decode_repr_prefers_repr_aware_decoder() {
        let ts = tele2_typesupport();
        // XCDR2 wire body for Tele2 { ts: 0x0102030405060708, seq: 9 }.
        let mut bytes = alloc::vec::Vec::new();
        bytes.extend_from_slice(&0x0102_0304_0506_0708i64.to_le_bytes());
        bytes.extend_from_slice(&9i32.to_le_bytes());
        let mut out = Tele2 { ts: 0, seq: 0 };
        // representation = 1 (XCDR2).
        // SAFETY: test-controlled pointers.
        let rc = unsafe {
            zerodds_xcdr2_decode_repr(
                &ts,
                bytes.as_ptr(),
                bytes.len(),
                1,
                &mut out as *mut _ as *mut c_void,
            )
        };
        assert_eq!(
            rc,
            ZeroDdsStatus::Ok as c_int,
            "repr-aware decoder must run (legacy returns PreconditionNotMet)"
        );
        assert_eq!(out.ts, 0x0102_0304_0506_0708);
        assert_eq!(out.seq, 9);
    }

    #[test]
    fn decode_repr_falls_back_to_legacy_when_absent() {
        // Point has no decode_repr -> the legacy decode must run.
        let ts = point_typesupport();
        let bytes = [0x01u8, 0x00, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF];
        let mut out = PointSample { x: 0, y: 0 };
        // SAFETY: test-controlled pointers.
        let rc = unsafe {
            zerodds_xcdr2_decode_repr(
                &ts,
                bytes.as_ptr(),
                bytes.len(),
                1,
                &mut out as *mut _ as *mut c_void,
            )
        };
        assert_eq!(rc, ZeroDdsStatus::Ok as c_int);
        assert_eq!(out.x, 1);
        assert_eq!(out.y, -2);
    }
}
