// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.8.2 through the C API: a sample written with
//! `zerodds_writer_write_related` reaches `zerodds_reader_take_wire` with the
//! related sample identity, the writer GUID and the sequence number of its
//! DATA.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::ffi::CString;
use std::ptr;

use zerodds::{
    ZeroDdsEndpointQos, zerodds_buffer_free, zerodds_endpoint_qos_default,
    zerodds_reader_create_qos, zerodds_reader_destroy, zerodds_reader_take_wire,
    zerodds_runtime_create, zerodds_runtime_destroy, zerodds_writer_create_qos,
    zerodds_writer_destroy, zerodds_writer_guid, zerodds_writer_write,
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
fn a_related_sample_identity_reaches_the_reader_with_the_wire_identity() {
    let lease = zerodds_test_domains::lease_domain();
    let topic = CString::new("CFfiRelatedIdentity").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    let related_guid = [0xA5u8; 16];
    // SAFETY: every pointer comes from the create call above its use and is
    // checked for NULL.
    unsafe {
        let rt = zerodds_runtime_create(lease.id() as u32);
        assert!(!rt.is_null());
        let writer = zerodds_writer_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &reliable());
        let reader = zerodds_reader_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &reliable());
        assert!(!writer.is_null() && !reader.is_null());
        let mut writer_guid = [0u8; 16];
        assert_eq!(zerodds_writer_guid(writer, writer_guid.as_mut_ptr()), 0);

        // A plain write carries no related identity.
        assert_eq!(zerodds_writer_write(writer, b"plain".as_ptr(), 5), 0);
        let mut sent_sn = 0i64;
        assert_eq!(
            zerodds_writer_write_related(
                writer,
                b"reply".as_ptr(),
                5,
                related_guid.as_ptr(),
                77,
                &mut sent_sn,
            ),
            0
        );
        assert!(sent_sn > 0, "the RTPS sequence number comes back");

        let mut taken = Vec::new();
        for _ in 0..200 {
            let mut buf: *mut u8 = ptr::null_mut();
            let mut len = 0usize;
            let mut guid = [0u8; 16];
            let mut sn = 0i64;
            let mut has = 9u8;
            let mut rguid = [0u8; 16];
            let mut rsn = 0u64;
            let rc = zerodds_reader_take_wire(
                reader,
                &mut buf,
                &mut len,
                ptr::null_mut(),
                ptr::null_mut(),
                guid.as_mut_ptr(),
                &mut sn,
                &mut has,
                rguid.as_mut_ptr(),
                &mut rsn,
            );
            assert_eq!(rc, 0);
            if buf.is_null() {
                if taken.len() == 2 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            let body = std::slice::from_raw_parts(buf, len).to_vec();
            zerodds_buffer_free(buf, len);
            taken.push((body, guid, sn, has, rguid, rsn));
        }
        assert_eq!(taken.len(), 2);
        let (body, guid, _sn, has, _, _) = &taken[0];
        assert_eq!(body, b"plain");
        assert_eq!(*guid, writer_guid);
        assert_eq!(*has, 0, "a plain write has no related identity");
        let (body, guid, sn, has, rguid, rsn) = &taken[1];
        assert_eq!(body, b"reply");
        assert_eq!(*guid, writer_guid);
        assert_eq!(*sn, sent_sn);
        assert_eq!(*has, 1);
        assert_eq!(*rguid, related_guid);
        assert_eq!(*rsn, 77);

        zerodds_reader_destroy(reader);
        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(rt);
    }
}
