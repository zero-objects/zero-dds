//! Smoke-Test: erzeuge Runtime + Writer + Reader, schreibe ein Sample,
//! lies es zurück. Ruft die `extern "C"`-Funktionen wie ein C-Caller auf.
//!
//! Kein echter C-Build hier — der C-Build ist in `examples/c_smoke.c`
//! als ergänzender Test. Dieser Rust-Test verifiziert, dass die FFI-
//! Schnittstelle in sich konsistent ist.

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
    zerodds_reader_return_loan, zerodds_reader_take, zerodds_runtime_create,
    zerodds_runtime_destroy, zerodds_runtime_wait_for_peers, zerodds_writer_create,
    zerodds_writer_destroy, zerodds_writer_wait_for_matched, zerodds_writer_write,
};

/// Pub-Sub-Roundtrip ueber das FFI mit ZWEI Participants (Pub + Sub).
/// Single-Runtime-Pub+Sub geht NICHT, weil `wire_writer_to_remote_reader`
/// SPDP-discovered Locators braucht — die existieren erst wenn ein
/// SEPARATER Participant via SPDP angekommen ist.
///
/// Auf macOS: ignored (Multicast-Loopback unzuverlaessig).
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn ffi_pub_sub_roundtrip() {
    let domain: u32 = 100 + (std::process::id() % 50);
    let topic = CString::new("CFfiSmoke").unwrap();
    let typ = CString::new("RawBytes").unwrap();

    // SAFETY: Test-FFI-Aufrufe mit valid C-Strings + NULL-checks gemaess
    // jeder pub unsafe fn # Safety. CStrings leben bis Block-Ende.
    unsafe {
        let rt_pub = zerodds_runtime_create(domain);
        let rt_sub = zerodds_runtime_create(domain);
        assert!(
            !rt_pub.is_null() && !rt_sub.is_null(),
            "runtime_create failed"
        );

        // Vor Endpoint-Erstellung: SPDP-Discovery abwarten. Sonst
        // wired wire_*_to_remote_* mit leerer Locator-Liste.
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
            let rc = zerodds_reader_take(reader, &mut buf, &mut len);
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

#[test]
fn ffi_handles_null_safely() {
    // SAFETY: alle destroy-Funktionen sind explizit NULL-tolerant
    // dokumentiert; NULL-Pointer als Argument ist by-design erlaubt.
    unsafe {
        // alle destroy-Funktionen müssen NULL-tolerant sein
        zerodds_runtime_destroy(ptr::null_mut());
        zerodds_writer_destroy(ptr::null_mut());
        zerodds_reader_destroy(ptr::null_mut());
        zerodds_buffer_free(ptr::null_mut(), 0);
        // Opt-1 R6: Loan-API ist auch NULL-tolerant.
        zerodds_reader_return_loan(ptr::null_mut());
    }
}

/// Opt-1 R6 — Read-Loan-API Roundtrip ohne to_vec().
///
/// Vergleich zu `ffi_pub_sub_roundtrip`: statt
/// `zerodds_reader_take` (mit interner `to_vec().into_boxed_slice()`)
/// verwendet der Subscriber `zerodds_reader_loan` + `_return_loan`.
/// Bei loan ist `*out_buf` ein direkter Pointer in das interne
/// `Arc<[u8]>` — Zero-Copy auf dem C-FFI-Boundary.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn ffi_pub_sub_roundtrip_via_loan() {
    let domain: u32 = 150 + (std::process::id() % 50);
    let topic = CString::new("CFfiSmokeLoan").unwrap();
    let typ = CString::new("RawBytes").unwrap();

    // SAFETY: gleiches Schema wie ffi_pub_sub_roundtrip — valid C-Strings,
    // NULL-checks gemaess jeder pub unsafe fn # Safety, CStrings leben
    // bis Block-Ende.
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
                // Lesen waehrend loan lebt — Bytes muessen die
                // erwarteten Werte tragen.
                let bytes = std::slice::from_raw_parts(buf, len);
                assert!(bytes.len() >= 4);
                assert_eq!(bytes[3], 0xAB, "Marker-Byte aus dem Payload");
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
