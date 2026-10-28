// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A data callback set after `zerodds_reader_create_qos` receives what the
//! reader already holds.
//!
//! Samples reach a reader before its caller can set a callback: a
//! TRANSIENT_LOCAL writer of the same runtime replays its retained samples
//! while the reader is created, and anything written before the `set` call
//! waits in the reader's channel. A callback-only consumer (every ROS 2
//! subscription of `rmw_zerodds`) never calls `take`, so before the backlog
//! was handed over these samples stayed in the channel for good.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::ffi::CString;
use std::ptr;
use std::sync::Mutex;

use core::ffi::c_void;

use zerodds::{
    ZeroDdsEndpointQos, zerodds_buffer_free, zerodds_endpoint_qos_default,
    zerodds_reader_create_qos, zerodds_reader_destroy, zerodds_reader_set_data_callback,
    zerodds_reader_take, zerodds_runtime_create, zerodds_runtime_destroy,
    zerodds_writer_create_qos, zerodds_writer_destroy, zerodds_writer_write,
};

extern "C" fn collect(ud: *mut c_void, payload: *const u8, len: usize, _repr: u8, _be: u8) {
    // SAFETY: `ud` is the `Mutex` each test keeps alive until the reader is
    // destroyed.
    let heard = unsafe { &*(ud as *const Mutex<Vec<Vec<u8>>>) };
    // SAFETY: `payload[..len]` is valid for the duration of the call.
    let body = unsafe { core::slice::from_raw_parts(payload, len) };
    heard.lock().unwrap().push(body.to_vec());
}

fn qos(transient_local: bool, keep_last: Option<u32>) -> ZeroDdsEndpointQos {
    ZeroDdsEndpointQos {
        reliable: 1,
        transient_local: i32::from(transient_local),
        history_keep_all: i32::from(keep_last.is_none()),
        history_depth: keep_last.unwrap_or(1),
        ..zerodds_endpoint_qos_default()
    }
}

/// Whatever `take` still finds: nothing, when the callback got it all.
///
/// # Safety
/// `reader` comes from `zerodds_reader_create_qos` and is not destroyed.
unsafe fn left_in_channel(reader: *mut zerodds::ZeroDdsReader) -> usize {
    let mut n = 0;
    loop {
        let mut buf: *mut u8 = ptr::null_mut();
        let mut len = 0usize;
        // SAFETY: valid reader and out pointers.
        let rc = unsafe { zerodds_reader_take(reader, &mut buf, &mut len, ptr::null_mut()) };
        if rc != 0 || buf.is_null() {
            return n;
        }
        // SAFETY: buffer handed out by the take above.
        unsafe { zerodds_buffer_free(buf, len) };
        n += 1;
    }
}

#[test]
fn a_retained_sample_of_the_same_runtime_reaches_a_callback_set_after_create() {
    let lease = zerodds_test_domains::lease_domain();
    let topic = CString::new("CFfiBacklogRetained").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    let heard: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    // SAFETY: every pointer comes from the create call above its use and is
    // checked for NULL; `heard` outlives the reader.
    unsafe {
        let rt = zerodds_runtime_create(lease.id() as u32);
        assert!(!rt.is_null(), "runtime create failed");
        let writer =
            zerodds_writer_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &qos(true, Some(1)));
        assert!(!writer.is_null());
        assert_eq!(zerodds_writer_write(writer, b"retained".as_ptr(), 8), 0);

        // The reader joins late: its creation replays the retained sample.
        let reader =
            zerodds_reader_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &qos(true, None));
        assert!(!reader.is_null());
        let ud = &heard as *const Mutex<Vec<Vec<u8>>> as *mut c_void;
        assert_eq!(
            zerodds_reader_set_data_callback(reader, Some(collect), ud),
            0
        );
        assert_eq!(
            *heard.lock().unwrap(),
            vec![b"retained".to_vec()],
            "the replayed sample reaches the callback during the set call"
        );

        assert_eq!(zerodds_writer_write(writer, b"live".as_ptr(), 4), 0);
        assert_eq!(
            *heard.lock().unwrap(),
            vec![b"retained".to_vec(), b"live".to_vec()]
        );
        assert_eq!(left_in_channel(reader), 0, "nothing stays in the channel");

        zerodds_reader_destroy(reader);
        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(rt);
    }
}

#[test]
fn samples_written_before_the_callback_reach_it_oldest_first_within_the_depth() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_all = CString::new("CFfiBacklogKeepAll").unwrap();
    let topic_last = CString::new("CFfiBacklogKeepLast").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    let heard_all: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    let heard_last: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    // SAFETY: as above.
    unsafe {
        let rt = zerodds_runtime_create(lease.id() as u32);
        assert!(!rt.is_null(), "runtime create failed");
        let w_all =
            zerodds_writer_create_qos(rt, topic_all.as_ptr(), typ.as_ptr(), 0, &qos(false, None));
        let w_last =
            zerodds_writer_create_qos(rt, topic_last.as_ptr(), typ.as_ptr(), 0, &qos(false, None));
        let r_all =
            zerodds_reader_create_qos(rt, topic_all.as_ptr(), typ.as_ptr(), 0, &qos(false, None));
        let r_last = zerodds_reader_create_qos(
            rt,
            topic_last.as_ptr(),
            typ.as_ptr(),
            0,
            &qos(false, Some(1)),
        );
        assert!(!w_all.is_null() && !w_last.is_null() && !r_all.is_null() && !r_last.is_null());

        for body in [b"one", b"two"] {
            assert_eq!(zerodds_writer_write(w_all, body.as_ptr(), 3), 0);
            assert_eq!(zerodds_writer_write(w_last, body.as_ptr(), 3), 0);
        }
        let ud_all = &heard_all as *const Mutex<Vec<Vec<u8>>> as *mut c_void;
        let ud_last = &heard_last as *const Mutex<Vec<Vec<u8>>> as *mut c_void;
        assert_eq!(
            zerodds_reader_set_data_callback(r_all, Some(collect), ud_all),
            0
        );
        assert_eq!(
            zerodds_reader_set_data_callback(r_last, Some(collect), ud_last),
            0
        );
        assert_eq!(
            *heard_all.lock().unwrap(),
            vec![b"one".to_vec(), b"two".to_vec()],
            "KEEP_ALL: every waiting sample, oldest first"
        );
        assert_eq!(
            *heard_last.lock().unwrap(),
            vec![b"two".to_vec()],
            "KEEP_LAST(1): the newest, as a take would have returned"
        );

        assert_eq!(zerodds_writer_write(w_all, b"six".as_ptr(), 3), 0);
        assert_eq!(heard_all.lock().unwrap().last().unwrap(), b"six");
        assert_eq!(left_in_channel(r_all), 0);
        assert_eq!(left_in_channel(r_last), 0);

        zerodds_reader_destroy(r_all);
        zerodds_reader_destroy(r_last);
        zerodds_writer_destroy(w_all);
        zerodds_writer_destroy(w_last);
        zerodds_runtime_destroy(rt);
    }
}
