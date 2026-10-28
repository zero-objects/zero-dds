// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `handle` of `zerodds_dw_write` and `zerodds_dw_write_w_timestamp` (DDS
//! 1.4 §2.2.2.4.2.11, §2.2.2.4.2.12): HANDLE_NIL, or the handle
//! register_instance returned for the key of the sample. Another existing
//! instance is PRECONDITION_NOT_MET, a handle that names none BAD_PARAMETER.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod typed_writer;

use std::ffi::CString;
use std::ptr;

use zerodds::dyntype_ffi::{
    zerodds_dyntype_bytes_free, zerodds_dyntype_destroy, zerodds_dyntype_encode_json,
};
use zerodds::extra_ffi::zerodds_dw_register_instance;
use zerodds::factory_ffi::zerodds_dpf_delete_participant;
use zerodds::publisher_ffi::{zerodds_dw_write, zerodds_dw_write_w_timestamp};

const OK: i32 = 0;
const BAD_PARAMETER: i32 = -6;
const PRECONDITION_NOT_MET: i32 = -5;

#[test]
fn a_write_handle_must_be_the_one_of_the_key() {
    let lease = zerodds_test_domains::lease_domain();
    // SAFETY: handles come from the create calls above their use and are
    // released at the end; the encoded buffers are freed after the write.
    unsafe {
        let typed_writer::TypedWriter {
            t, f, dp, writer, ..
        } = typed_writer::typed_writer(
            "@final struct HRow { @key long id; double v; };",
            "HRow",
            "CWriteHandle",
            lease.id() as u32,
        );

        // The KeyHash of id 7 and of id 8: the four BE octets, zero-padded.
        let register = |id: i32| {
            let mut spelled = [0u8; 16];
            spelled[..4].copy_from_slice(&id.to_be_bytes());
            let mut handle = 0u64;
            assert_eq!(
                zerodds_dw_register_instance(writer, spelled.as_ptr(), 16, &mut handle),
                OK
            );
            handle
        };
        let h7 = register(7);
        let h8 = register(8);
        assert_ne!(h7, h8);

        let write = |id: i32, handle: u64, stamped: bool| {
            let json = CString::new(format!("{{\"id\":{id},\"v\":1.0}}")).unwrap();
            let mut bytes = ptr::null_mut();
            let mut size = 0usize;
            assert_eq!(
                zerodds_dyntype_encode_json(t, json.as_ptr(), 0, 0, &mut bytes, &mut size),
                OK
            );
            let rc = if stamped {
                zerodds_dw_write_w_timestamp(writer, bytes, size, handle, 5, 0)
            } else {
                zerodds_dw_write(writer, bytes, size, handle)
            };
            zerodds_dyntype_bytes_free(bytes, size);
            rc
        };

        assert_eq!(write(7, 0, false), OK, "HANDLE_NIL");
        assert_eq!(write(7, h7, false), OK, "the handle of the key");
        assert_eq!(write(8, h8, true), OK, "the handle of the key, stamped");
        assert_eq!(write(7, h8, false), PRECONDITION_NOT_MET);
        assert_eq!(write(8, h7, true), PRECONDITION_NOT_MET);
        assert_eq!(write(7, 0xDEAD_BEEF, false), BAD_PARAMETER);
        assert_eq!(write(7, 0xDEAD_BEEF, true), BAD_PARAMETER);

        zerodds_dyntype_destroy(t);
        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}
