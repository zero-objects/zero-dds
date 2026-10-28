// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.5.1.1.1 through the C API: `requestId` is the identity of
//! the sample the header is part of, and the header is filled before the
//! write. `zerodds_writer_next_sequence_number` gives the RTPS sequence number
//! (DDSI-RTPS 2.5 §8.3.5.4) the next write will carry, so that the number in
//! the header is the one the reader sees.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::ffi::CString;
use std::ptr;

use zerodds::{
    ZeroDdsEndpointQos, zerodds_buffer_free, zerodds_endpoint_qos_default,
    zerodds_reader_create_qos, zerodds_reader_destroy, zerodds_reader_take_wire,
    zerodds_runtime_create, zerodds_runtime_destroy, zerodds_writer_create_qos,
    zerodds_writer_destroy, zerodds_writer_next_sequence_number, zerodds_writer_write,
    zerodds_writer_write_related,
};

fn reliable() -> ZeroDdsEndpointQos {
    ZeroDdsEndpointQos {
        reliable: 1,
        history_keep_all: 1,
        ..zerodds_endpoint_qos_default()
    }
}

#[test]
fn the_next_sequence_number_is_the_one_the_next_write_carries() {
    let lease = zerodds_test_domains::lease_domain();
    let topic = CString::new("CFfiNextSequenceNumber").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    // SAFETY: every pointer comes from the create call above its use and is
    // checked for NULL.
    unsafe {
        let rt = zerodds_runtime_create(lease.id() as u32);
        assert!(!rt.is_null());
        let writer = zerodds_writer_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &reliable());
        let reader = zerodds_reader_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &reliable());
        assert!(!writer.is_null() && !reader.is_null());

        // Nothing written: the first sample is number 1, however often asked.
        let mut next = 0i64;
        assert_eq!(zerodds_writer_next_sequence_number(writer, &mut next), 0);
        assert_eq!(next, 1);
        assert_eq!(zerodds_writer_next_sequence_number(writer, &mut next), 0);
        assert_eq!(next, 1, "asking does not consume the number");

        // A plain write takes it, and the one after is the next.
        assert_eq!(zerodds_writer_write(writer, b"first".as_ptr(), 5), 0);
        assert_eq!(zerodds_writer_next_sequence_number(writer, &mut next), 0);
        assert_eq!(next, 2);

        // The number promised is the number of the sample, whichever write
        // sends it: the related write reports the one it used.
        let promised = next;
        let related_guid = [0x5Au8; 16];
        let mut used = 0i64;
        assert_eq!(
            zerodds_writer_write_related(
                writer,
                b"second".as_ptr(),
                6,
                related_guid.as_ptr(),
                1,
                &mut used,
            ),
            0
        );
        assert_eq!(used, promised);
        assert_eq!(zerodds_writer_next_sequence_number(writer, &mut next), 0);
        assert_eq!(next, promised + 1);

        // And it is the number the reader sees on the DATA.
        let promised = next;
        assert_eq!(zerodds_writer_write(writer, b"third".as_ptr(), 5), 0);
        let mut seen = Vec::new();
        for _ in 0..200 {
            let mut buf: *mut u8 = ptr::null_mut();
            let mut len = 0usize;
            let mut sn = 0i64;
            let rc = zerodds_reader_take_wire(
                reader,
                &mut buf,
                &mut len,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut sn,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            assert_eq!(rc, 0);
            if buf.is_null() {
                if seen.len() == 3 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            let body = std::slice::from_raw_parts(buf, len).to_vec();
            zerodds_buffer_free(buf, len);
            seen.push((body, sn));
        }
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[2].0, b"third");
        assert_eq!(seen[2].1, promised);

        // A handle that is not one.
        assert_eq!(
            zerodds_writer_next_sequence_number(ptr::null_mut(), &mut next),
            -2
        );
        assert_eq!(
            zerodds_writer_next_sequence_number(writer, ptr::null_mut()),
            -2
        );

        zerodds_reader_destroy(reader);
        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(rt);
    }
}
