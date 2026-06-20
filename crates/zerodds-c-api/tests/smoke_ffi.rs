//! Smoke test: create runtime + writer + reader, write a sample,
//! read it back. Calls the `extern "C"` functions like a C caller.
//!
//! No real C build here — the C build is in `examples/c_smoke.c`
//! as a complementary test. This Rust test verifies that the FFI
//! interface is internally consistent.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::ffi::CString;
use std::ptr;
use std::thread;
use std::time::Duration;

use core::ffi::c_void;

use zerodds::{
    zerodds_buffer_free, zerodds_reader_create, zerodds_reader_destroy, zerodds_reader_loan,
    zerodds_reader_return_loan, zerodds_reader_take, zerodds_reader_take_into,
    zerodds_runtime_create, zerodds_runtime_destroy, zerodds_runtime_wait_for_peers,
    zerodds_writer_create, zerodds_writer_destroy, zerodds_writer_wait_for_matched,
    zerodds_writer_write,
};

/// Pub-sub roundtrip over the FFI with TWO participants (pub + sub).
/// Single-runtime pub+sub does NOT work, because `wire_writer_to_remote_reader`
/// needs SPDP-discovered locators — those exist only once a
/// SEPARATE participant has arrived via SPDP.
///
/// On macOS: ignored (multicast loopback unreliable).
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn ffi_pub_sub_roundtrip() {
    let domain: u32 = 100 + (std::process::id() % 50);
    let topic = CString::new("CFfiSmoke").unwrap();
    let typ = CString::new("RawBytes").unwrap();

    // SAFETY: test FFI calls with valid C strings + NULL checks per
    // each pub unsafe fn # Safety. The CStrings live until the end of the block.
    unsafe {
        let rt_pub = zerodds_runtime_create(domain);
        let rt_sub = zerodds_runtime_create(domain);
        assert!(
            !rt_pub.is_null() && !rt_sub.is_null(),
            "runtime_create failed"
        );

        // Before creating endpoints: wait for SPDP discovery. Otherwise
        // wire_*_to_remote_* wires up with an empty locator list.
        assert_eq!(
            zerodds_runtime_wait_for_peers(rt_pub, 1, 5_000),
            0,
            "rt_pub did not see rt_sub via SPDP"
        );
        assert_eq!(
            zerodds_runtime_wait_for_peers(rt_sub, 1, 5_000),
            0,
            "rt_sub did not see rt_pub via SPDP"
        );

        let writer = zerodds_writer_create(rt_pub, topic.as_ptr(), typ.as_ptr(), 1);
        let reader = zerodds_reader_create(rt_sub, topic.as_ptr(), typ.as_ptr(), 1);
        assert!(!writer.is_null() && !reader.is_null());

        let _ = zerodds_writer_wait_for_matched(writer, 1, 5_000);

        for i in 0..5u8 {
            let payload = [i, i + 1, i + 2, 0xAB];
            let rc = zerodds_writer_write(writer, payload.as_ptr(), payload.len());
            assert_eq!(rc, 0, "write returned {rc}");
        }

        let mut received = 0;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while received < 5 && std::time::Instant::now() < deadline {
            let mut buf: *mut u8 = ptr::null_mut();
            let mut len: usize = 0;
            let rc = zerodds_reader_take(reader, &mut buf, &mut len, ptr::null_mut());
            assert_eq!(rc, 0);
            if !buf.is_null() && len > 0 {
                zerodds_buffer_free(buf, len);
                received += 1;
            } else {
                thread::sleep(Duration::from_millis(20));
            }
        }

        zerodds_writer_destroy(writer);
        zerodds_reader_destroy(reader);
        zerodds_runtime_destroy(rt_pub);
        zerodds_runtime_destroy(rt_sub);

        assert!(received >= 1, "expected ≥1 sample, got {received}");
    }
}

/// Fixed-buffer `zerodds_reader_take_into` roundtrip — the ergonomic
/// shape used by the C/Go/Zig quickstarts. Mirrors
/// `ffi_pub_sub_roundtrip` but consumes samples via the copy-into-buffer
/// convenience entry point (no heap buffer / no `zerodds_buffer_free`).
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn ffi_pub_sub_roundtrip_via_take_into() {
    let domain: u32 = 170 + (std::process::id() % 50);
    let topic = CString::new("CFfiSmokeTakeInto").unwrap();
    let typ = CString::new("RawBytes").unwrap();

    // SAFETY: valid C strings; pointers NULL-checked per each fn # Safety.
    unsafe {
        let rt_pub = zerodds_runtime_create(domain);
        let rt_sub = zerodds_runtime_create(domain);
        assert!(!rt_pub.is_null() && !rt_sub.is_null());

        assert_eq!(zerodds_runtime_wait_for_peers(rt_pub, 1, 5_000), 0);
        assert_eq!(zerodds_runtime_wait_for_peers(rt_sub, 1, 5_000), 0);

        let writer = zerodds_writer_create(rt_pub, topic.as_ptr(), typ.as_ptr(), 1);
        let reader = zerodds_reader_create(rt_sub, topic.as_ptr(), typ.as_ptr(), 1);
        assert!(!writer.is_null() && !reader.is_null());

        let _ = zerodds_writer_wait_for_matched(writer, 1, 5_000);

        let payload = [0xDEu8, 0xAD, 0xBE, 0xEF];
        for _ in 0..5u8 {
            let rc = zerodds_writer_write(writer, payload.as_ptr(), payload.len());
            assert_eq!(rc, 0);
        }

        let mut received = 0;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while received < 5 && std::time::Instant::now() < deadline {
            let mut buf = [0u8; 64];
            let n = zerodds_reader_take_into(reader, buf.as_mut_ptr(), buf.len());
            assert!(n >= 0, "take_into returned negative {n}");
            if n > 0 {
                let n = n as usize;
                assert_eq!(&buf[..n], &payload[..], "payload mismatch");
                received += 1;
            } else {
                thread::sleep(Duration::from_millis(20));
            }
        }

        zerodds_writer_destroy(writer);
        zerodds_reader_destroy(reader);
        zerodds_runtime_destroy(rt_pub);
        zerodds_runtime_destroy(rt_sub);

        assert!(received >= 1, "expected ≥1 sample, got {received}");
    }
}

/// `zerodds_reader_take_into` is NULL-safe on the handle and returns a
/// negative status rather than UB.
#[test]
fn ffi_take_into_null_safe() {
    // SAFETY: NULL handle is explicitly handled; buf may be NULL iff cap==0.
    unsafe {
        let n = zerodds_reader_take_into(ptr::null_mut(), ptr::null_mut(), 0);
        assert!(n < 0, "NULL reader must yield a negative status, got {n}");
    }
}

#[test]
fn ffi_handles_null_safely() {
    // SAFETY: all destroy functions are explicitly documented as
    // NULL-tolerant; a NULL pointer as an argument is allowed by design.
    unsafe {
        // all destroy functions must be NULL-tolerant
        zerodds_runtime_destroy(ptr::null_mut());
        zerodds_writer_destroy(ptr::null_mut());
        zerodds_reader_destroy(ptr::null_mut());
        zerodds_buffer_free(ptr::null_mut(), 0);
        // Opt-1 R6: the loan API is also NULL-tolerant.
        zerodds_reader_return_loan(ptr::null_mut());
    }
}

/// Opt-1 R6 — read-loan API roundtrip without to_vec().
///
/// Compared to `ffi_pub_sub_roundtrip`: instead of
/// `zerodds_reader_take` (with its internal `to_vec().into_boxed_slice()`)
/// the subscriber uses `zerodds_reader_loan` + `_return_loan`.
/// With loan `*out_buf` is a direct pointer into the internal
/// `Arc<[u8]>` — zero-copy at the C-FFI boundary.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn ffi_pub_sub_roundtrip_via_loan() {
    let domain: u32 = 150 + (std::process::id() % 50);
    let topic = CString::new("CFfiSmokeLoan").unwrap();
    let typ = CString::new("RawBytes").unwrap();

    // SAFETY: same scheme as ffi_pub_sub_roundtrip — valid C strings,
    // NULL checks per each pub unsafe fn # Safety, the CStrings live
    // until the end of the block.
    unsafe {
        let rt_pub = zerodds_runtime_create(domain);
        let rt_sub = zerodds_runtime_create(domain);
        assert!(!rt_pub.is_null() && !rt_sub.is_null());
        assert_eq!(zerodds_runtime_wait_for_peers(rt_pub, 1, 5_000), 0);
        assert_eq!(zerodds_runtime_wait_for_peers(rt_sub, 1, 5_000), 0);

        let writer = zerodds_writer_create(rt_pub, topic.as_ptr(), typ.as_ptr(), 1);
        let reader = zerodds_reader_create(rt_sub, topic.as_ptr(), typ.as_ptr(), 1);
        assert!(!writer.is_null() && !reader.is_null());

        let _ = zerodds_writer_wait_for_matched(writer, 1, 5_000);

        for i in 0..5u8 {
            let payload = [i, i + 1, i + 2, 0xAB];
            assert_eq!(
                zerodds_writer_write(writer, payload.as_ptr(), payload.len()),
                0
            );
        }

        let mut received = 0;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while received < 5 && std::time::Instant::now() < deadline {
            let mut buf: *const u8 = ptr::null();
            let mut len: usize = 0;
            let mut loan: *mut c_void = ptr::null_mut();
            let rc = zerodds_reader_loan(reader, &mut buf, &mut len, &mut loan);
            assert_eq!(rc, 0);
            if !buf.is_null() && len > 0 && !loan.is_null() {
                // Read while the loan lives — the bytes must carry the
                // expected values.
                let bytes = std::slice::from_raw_parts(buf, len);
                assert!(bytes.len() >= 4);
                assert_eq!(bytes[3], 0xAB, "marker byte from the payload");
                zerodds_reader_return_loan(loan);
                received += 1;
            } else {
                thread::sleep(Duration::from_millis(20));
            }
        }

        zerodds_writer_destroy(writer);
        zerodds_reader_destroy(reader);
        zerodds_runtime_destroy(rt_pub);
        zerodds_runtime_destroy(rt_sub);

        assert!(received >= 1, "expected ≥1 sample, got {received}");
    }
}

/// Regression: A2 bench bug 2026-05-19. RTPS spec §9.6.1.4.1 says
/// `spdp_port = 7400 + 250 * domain`. For domain ≥ 233 the
/// port exceeds u16::MAX. `DcpsRuntime::start` returns Err. The FFI must
/// NOT silently swallow that into a NULL without diagnosis — the A2 bench
/// did exactly that and then only died at wait_for_peers with a
/// cryptic BadHandle (-2).
///
/// This test ensures that:
/// * `runtime_create(250)` returns NULL (no UB, no crash)
/// * follow-up calls with the NULL handle return BadHandle (-2),
///   not segfault
#[test]
fn ffi_runtime_create_domain_too_large_returns_null() {
    // Domain 250 → spdp_port = 7400 + 250*250 = 69900 > u16::MAX
    // SAFETY: zerodds_runtime_create takes only a domain integer,
    // no pointers — no precondition risk.
    let rt = unsafe { zerodds_runtime_create(250) };
    assert!(
        rt.is_null(),
        "runtime_create(250) must return NULL (port overflow)"
    );
    // A NULL handle in a follow-up API must return BadHandle (-2), not crash
    // SAFETY: `rt` is NULL here (create returned NULL) — the NULL-handle path
    // is an explicit contract (returns BadHandle).
    let rc = unsafe { zerodds_runtime_wait_for_peers(rt, 1, 100) };
    assert_eq!(rc, -2, "wait_for_peers(NULL) must return BadHandle (-2)");
    // destroy(NULL) must be a no-op
    // SAFETY: `rt` comes from zerodds_runtime_create or is NULL —
    // destroy accepts both.
    unsafe { zerodds_runtime_destroy(rt) };
}

/// Domain 232 is the highest safe value (spdp_port = 65400 < u16::MAX).
#[test]
fn ffi_runtime_create_domain_232_succeeds() {
    // SAFETY: only a domain integer, no pointer precondition.
    let rt = unsafe { zerodds_runtime_create(232) };
    assert!(
        !rt.is_null(),
        "runtime_create(232) must be OK (port = 65400, max u16-safe)"
    );
    // SAFETY: `rt` comes from zerodds_runtime_create or is NULL —
    // destroy accepts both.
    unsafe { zerodds_runtime_destroy(rt) };
}

/// Domain 233 is the first overflow (spdp_port = 65650 > u16::MAX).
#[test]
fn ffi_runtime_create_domain_233_returns_null() {
    // SAFETY: only a domain integer, no pointer precondition.
    let rt = unsafe { zerodds_runtime_create(233) };
    assert!(
        rt.is_null(),
        "runtime_create(233) must return NULL (port = 65650 > u16::MAX)"
    );
    // SAFETY: `rt` comes from zerodds_runtime_create or is NULL —
    // destroy accepts both.
    unsafe { zerodds_runtime_destroy(rt) };
}
