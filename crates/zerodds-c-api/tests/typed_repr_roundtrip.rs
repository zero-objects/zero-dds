// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bug R4b regression: the C-API TYPED path carries the writer's actual
//! XCDR representation end-to-end.
//!
//! Before the fix, `zerodds_reader_take_typed` discarded the reader's
//! `out_repr` (passed `NULL`) and the decoder signature had no way to
//! receive the representation. A typed writer offering XCDR2 then had its
//! representation lost on the same-runtime typed read: a 64-bit-leading
//! multi-member sample would be decoded with the wrong `max_align`
//! (8 = XCDR1 instead of 4 = XCDR2) and underrun.
//!
//! This test drives the exact topology of the language bindings'
//! `TypedWriter<T>`/`TypedReader<T>` (one runtime shared by writer +
//! reader) through `zerodds_writer_write_typed` +
//! `zerodds_reader_take_typed`, and asserts the repr-aware decoder is
//! handed `representation == 1` (XCDR2) and round-trips a multi-member
//! sample without underrun.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use core::ffi::{c_char, c_int, c_void};
use core::sync::atomic::{AtomicU8, Ordering};
use std::ffi::CString;
use std::time::{Duration, Instant};

use zerodds::xcdr2::{
    ZeroDdsTypeSupport, copy_to_out_buf, input_slice, zerodds_reader_take_typed,
    zerodds_writer_write_typed,
};
use zerodds::{zerodds_reader_create, zerodds_runtime_create, zerodds_writer_create};

#[repr(C)]
struct Telemetry {
    ts: i64, // 64-bit lead member — the alignment-sensitive one
    seq: i32,
    flag: i32,
}

/// XCDR2 encoder (max_align 4): the stream starts 4-aligned, so the i64
/// sits at offset 0 with no leading pad, then two i32 follow contiguously
/// — 16 bytes total. (No DHEADER here: this raw-body encoder mirrors what
/// the runtime wraps with its own encap header for @final/simple types.)
// SAFETY: FFI-boundary; pointers caller-validated.
unsafe extern "C" fn tele_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: test-only.
    let s = unsafe { &*(sample as *const Telemetry) };
    let mut b = Vec::with_capacity(16);
    b.extend_from_slice(&s.ts.to_le_bytes());
    b.extend_from_slice(&s.seq.to_le_bytes());
    b.extend_from_slice(&s.flag.to_le_bytes());
    // SAFETY: helper.
    unsafe { copy_to_out_buf(&b, out_buf, out_cap, out_len) }
}

/// Records the representation the take path forwarded, so the test can
/// assert it was the publisher's actual XCDR version (not a lost 0).
static LAST_REPR: AtomicU8 = AtomicU8::new(0xEE);

/// Representation-aware decoder. For XCDR1 (`representation == 0`) the
/// i64 aligns to 8; for XCDR2 (`representation == 1`) alignment caps at 4.
/// A version-blind decoder defaulting to XCDR1's max_align=8 on a body
/// that is only 4-aligned would skip 4 phantom pad bytes and underrun the
/// 16-byte XCDR2 stream. Here we honor the handed-in representation.
// SAFETY: FFI-boundary; pointers caller-validated.
unsafe extern "C" fn tele_decode_repr(
    buf: *const u8,
    len: usize,
    representation: u8,
    out_sample: *mut c_void,
) -> c_int {
    LAST_REPR.store(representation, Ordering::SeqCst);
    // SAFETY: test-only.
    let b = unsafe { input_slice(buf, len) };
    let max_align: usize = if representation == 1 { 4 } else { 8 };
    // i64 offset within a 4-aligned body.
    let i64_off = (max_align - (0 % max_align)) % max_align; // 0 for both here
    let need = i64_off + 8 + 4 + 4;
    if b.len() < need {
        // A version-blind XCDR1 default on a non-8-aligned body would land
        // here as an underrun. With the correct representation forwarded,
        // the 16-byte XCDR2 stream is sufficient.
        return -7;
    }
    let ts = i64::from_le_bytes(b[i64_off..i64_off + 8].try_into().unwrap());
    let seq = i32::from_le_bytes(b[i64_off + 8..i64_off + 12].try_into().unwrap());
    let flag = i32::from_le_bytes(b[i64_off + 12..i64_off + 16].try_into().unwrap());
    // SAFETY: test-only.
    unsafe {
        let o = &mut *(out_sample as *mut Telemetry);
        o.ts = ts;
        o.seq = seq;
        o.flag = flag;
    }
    0
}

static TELE_NAME: &[u8] = b"Telemetry\0";

fn tele_typesupport() -> ZeroDdsTypeSupport {
    ZeroDdsTypeSupport {
        type_hash: [0u8; 16],
        type_name: TELE_NAME.as_ptr() as *const c_char,
        is_keyed: 0,
        extensibility: 0, // @final
        _reserved: [0u8; 6],
        encode: Some(tele_encode),
        decode: None, // force the repr-aware path
        key_hash: None,
        sample_free: None,
        decode_repr: Some(tele_decode_repr),
    }
}

/// Same-runtime typed writer→reader: the writer's negotiated
/// representation (XCDR2 by the default offer) must reach the typed
/// decoder via `zerodds_reader_take_typed`.
#[test]
fn typed_take_carries_writer_representation() {
    let topic = CString::new("R4bTelemetry").unwrap();
    let typ = CString::new("Telemetry").unwrap();
    let support = tele_typesupport();

    // SAFETY: valid C strings + NULL-checked FFI per each fn # Safety.
    unsafe {
        let rt = zerodds_runtime_create(181);
        assert!(!rt.is_null(), "runtime_create");
        let writer = zerodds_writer_create(rt, topic.as_ptr(), typ.as_ptr(), 0);
        let reader = zerodds_reader_create(rt, topic.as_ptr(), typ.as_ptr(), 0);
        assert!(!writer.is_null() && !reader.is_null());

        let sample = Telemetry {
            ts: 0x0102_0304_0506_0708,
            seq: 1234,
            flag: -9,
        };
        let rc = zerodds_writer_write_typed(writer, &support, &sample as *const _ as *const c_void);
        assert_eq!(rc, 0, "write_typed rc={rc}");

        // Same-runtime delivery: poll the typed take.
        let mut out = Telemetry {
            ts: 0,
            seq: 0,
            flag: 0,
        };
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut got = false;
        while Instant::now() < deadline {
            let rc = zerodds_reader_take_typed(
                reader,
                &support,
                &mut out as *mut _ as *mut c_void,
                core::ptr::null_mut(),
            );
            if rc == 0 {
                got = true;
                break;
            }
            // NoData -> retry.
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(got, "typed reader received no sample");
        // The crux of Bug R4b: the decoder was handed the writer's REAL
        // representation (XCDR2 = 1), not a lost 0.
        assert_eq!(
            LAST_REPR.load(Ordering::SeqCst),
            1,
            "take_typed must forward the writer's XCDR2 representation (Bug R4b); got {}",
            LAST_REPR.load(Ordering::SeqCst)
        );
        // And the multi-member sample round-tripped without underrun.
        assert_eq!(out.ts, 0x0102_0304_0506_0708);
        assert_eq!(out.seq, 1234);
        assert_eq!(out.flag, -9);
    }
}
