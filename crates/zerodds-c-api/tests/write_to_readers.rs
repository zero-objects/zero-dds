// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.11.1.4.3 through the C API: "Requests sent through a bound
//! requester reference shall be sent to the bound service instance only."
//! `zerodds_writer_write_to_readers` writes a sample for the readers it names
//! and no other: two readers of one runtime on one topic, one of them named.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::ffi::CString;
use std::ptr;

use zerodds::{
    ZeroDdsEndpointQos, ZeroDdsReader, zerodds_buffer_free, zerodds_endpoint_qos_default,
    zerodds_reader_create_qos, zerodds_reader_destroy, zerodds_reader_guid,
    zerodds_reader_take_wire, zerodds_runtime_create, zerodds_runtime_destroy,
    zerodds_writer_create_qos, zerodds_writer_destroy, zerodds_writer_write,
    zerodds_writer_write_to_readers,
};

fn reliable() -> ZeroDdsEndpointQos {
    ZeroDdsEndpointQos {
        reliable: 1,
        history_keep_all: 1,
        ..zerodds_endpoint_qos_default()
    }
}

/// Everything `reader` has, as the payloads of its samples.
///
/// # Safety
/// `reader` is a live reader.
unsafe fn drain(reader: *mut ZeroDdsReader) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let mut buf: *mut u8 = ptr::null_mut();
        let mut len = 0usize;
        // SAFETY: the caller's guarantee; the out parameters are locals.
        let rc = unsafe {
            zerodds_reader_take_wire(
                reader,
                &mut buf,
                &mut len,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        assert_eq!(rc, 0);
        if buf.is_null() {
            return out;
        }
        // SAFETY: `buf` and `len` come from the take above.
        unsafe {
            out.push(std::slice::from_raw_parts(buf, len).to_vec());
            zerodds_buffer_free(buf, len);
        }
    }
}

#[test]
fn a_sample_written_to_one_reader_reaches_that_reader_only() {
    let lease = zerodds_test_domains::lease_domain();
    let topic = CString::new("CFfiWriteToReaders").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    // SAFETY: every pointer comes from the create call above its use and is
    // checked for NULL.
    unsafe {
        let rt = zerodds_runtime_create(lease.id() as u32);
        assert!(!rt.is_null());
        let writer = zerodds_writer_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &reliable());
        let first = zerodds_reader_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &reliable());
        let second = zerodds_reader_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &reliable());
        assert!(!writer.is_null() && !first.is_null() && !second.is_null());
        let mut first_guid = [0u8; 16];
        let mut second_guid = [0u8; 16];
        assert_eq!(zerodds_reader_guid(first, first_guid.as_mut_ptr()), 0);
        assert_eq!(zerodds_reader_guid(second, second_guid.as_mut_ptr()), 0);
        assert_ne!(first_guid, second_guid);

        let mut to_first = 0i64;
        let mut to_second = 0i64;
        let mut to_nobody = 0i64;
        assert_eq!(
            zerodds_writer_write_to_readers(
                writer,
                b"for-first".as_ptr(),
                9,
                first_guid.as_ptr(),
                1,
                &mut to_first,
            ),
            0
        );
        assert_eq!(
            zerodds_writer_write_to_readers(
                writer,
                b"for-second".as_ptr(),
                10,
                second_guid.as_ptr(),
                1,
                &mut to_second,
            ),
            0
        );
        assert_eq!(
            zerodds_writer_write_to_readers(
                writer,
                b"for-nobody".as_ptr(),
                10,
                ptr::null(),
                0,
                &mut to_nobody,
            ),
            0
        );
        assert_eq!(zerodds_writer_write(writer, b"for-all".as_ptr(), 7), 0);
        assert!(
            0 < to_first && to_first < to_second && to_second < to_nobody,
            "each write used a sequence number: {to_first} {to_second} {to_nobody}"
        );

        // The last sample goes to both, and the writer is in order, so
        // when a reader has it, it has had everything before it.
        let mut at_first = Vec::new();
        let mut at_second = Vec::new();
        for _ in 0..300 {
            at_first.extend(drain(first));
            at_second.extend(drain(second));
            let done = |seen: &Vec<Vec<u8>>| seen.last().is_some_and(|s| s == b"for-all");
            if done(&at_first) && done(&at_second) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            at_first,
            vec![b"for-first".to_vec(), b"for-all".to_vec()],
            "the first reader gets what is for it and for all"
        );
        assert_eq!(
            at_second,
            vec![b"for-second".to_vec(), b"for-all".to_vec()],
            "and so does the second"
        );

        // A handle that is not one, a list that is not there.
        assert_eq!(
            zerodds_writer_write_to_readers(
                ptr::null_mut(),
                b"x".as_ptr(),
                1,
                first_guid.as_ptr(),
                1,
                ptr::null_mut(),
            ),
            -2
        );
        assert_eq!(
            zerodds_writer_write_to_readers(
                writer,
                b"x".as_ptr(),
                1,
                ptr::null(),
                1,
                ptr::null_mut()
            ),
            -2
        );

        zerodds_reader_destroy(first);
        zerodds_reader_destroy(second);
        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(rt);
    }
}
