// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The arrival callback tells a reader's owner that samples are waiting and
//! leaves them in the reader (DDS-RPC 1.0 §7.9.2.2): a listener can take the
//! request and write the reply from its own thread.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use core::ffi::c_void;
use std::ffi::CString;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use zerodds::arrival_ffi::zerodds_reader_set_arrival_callback;
use zerodds::{
    ZeroDdsEndpointQos, zerodds_buffer_free, zerodds_endpoint_qos_default,
    zerodds_reader_create_qos, zerodds_reader_destroy, zerodds_reader_take, zerodds_runtime_create,
    zerodds_runtime_destroy, zerodds_writer_create_qos, zerodds_writer_destroy,
    zerodds_writer_write,
};

static SIGNALS: AtomicUsize = AtomicUsize::new(0);
static RELEASES: AtomicUsize = AtomicUsize::new(0);

extern "C" fn signalled(_user_data: *mut c_void) {
    SIGNALS.fetch_add(1, Ordering::SeqCst);
}

extern "C" fn released(_user_data: *mut c_void) {
    RELEASES.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn a_signal_follows_a_run_of_samples_and_the_samples_stay_in_the_reader() {
    let lease = zerodds_test_domains::lease_domain();
    let topic = CString::new("CFfiArrivalCallback").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    let qos = ZeroDdsEndpointQos {
        reliable: 1,
        history_keep_all: 1,
        ..zerodds_endpoint_qos_default()
    };
    // SAFETY: every pointer comes from the create call above its use and is
    // checked for NULL.
    unsafe {
        let rt = zerodds_runtime_create(lease.id() as u32);
        assert!(!rt.is_null());
        let writer = zerodds_writer_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &qos);
        let reader = zerodds_reader_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &qos);
        assert!(!writer.is_null() && !reader.is_null());
        assert_eq!(
            zerodds_reader_set_arrival_callback(
                reader,
                Some(signalled),
                ptr::null_mut(),
                Some(released)
            ),
            0
        );

        assert_eq!(zerodds_writer_write(writer, b"one".as_ptr(), 3), 0);
        for _ in 0..200 {
            if SIGNALS.load(Ordering::SeqCst) > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            SIGNALS.load(Ordering::SeqCst) > 0,
            "the arrival was signalled"
        );

        // The sample was not consumed by the signal.
        let mut buf: *mut u8 = ptr::null_mut();
        let mut len = 0usize;
        assert_eq!(
            zerodds_reader_take(reader, &mut buf, &mut len, ptr::null_mut()),
            0
        );
        assert!(!buf.is_null());
        assert_eq!(std::slice::from_raw_parts(buf, len), b"one");
        zerodds_buffer_free(buf, len);

        // Removing the callback releases its user data once, and signals no more.
        assert_eq!(RELEASES.load(Ordering::SeqCst), 0);
        assert_eq!(
            zerodds_reader_set_arrival_callback(reader, None, ptr::null_mut(), None),
            0
        );
        assert_eq!(RELEASES.load(Ordering::SeqCst), 1);
        let before = SIGNALS.load(Ordering::SeqCst);
        assert_eq!(zerodds_writer_write(writer, b"two".as_ptr(), 3), 0);
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(
            SIGNALS.load(Ordering::SeqCst),
            before,
            "no signal after removal"
        );

        zerodds_reader_destroy(reader);
        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(rt);
    }
}
