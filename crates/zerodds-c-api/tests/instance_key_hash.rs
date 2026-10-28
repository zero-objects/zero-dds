// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds_dw_instance_key_hash` gives the key hash of the instance a
//! handle names, so that `dispose_instance(handle)` and
//! `unregister_instance(handle)` of the C++ API (DDS-PSM-Cxx 1.0 §7.9) reach
//! `zerodds_dw_dispose` with the instance's key.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::CString;
use std::ptr;

use zerodds::extra_ffi::zerodds_dw_register_instance;
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::instance_ffi::zerodds_dw_instance_key_hash;
use zerodds::participant_ffi::{zerodds_dp_create_publisher, zerodds_dp_create_topic};
use zerodds::publisher_ffi::zerodds_pub_create_datawriter;

const OK: i32 = 0;
const BAD_PARAMETER: i32 = -6;

#[test]
fn the_key_hash_of_a_registered_instance_comes_back_and_an_unknown_handle_is_refused() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CInstanceKeyHash").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    // SAFETY: every pointer comes from the create call above its use and is
    // checked; the participant is deleted once at the end.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        assert!(!writer.is_null());

        let key: [u8; 16] = *b"0123456789abcdef";
        let mut handle = 0u64;
        assert_eq!(
            zerodds_dw_register_instance(writer, key.as_ptr(), 16, &mut handle),
            OK
        );
        assert_ne!(handle, 0);

        let mut out = [0u8; 16];
        assert_eq!(
            zerodds_dw_instance_key_hash(writer, handle, out.as_mut_ptr()),
            OK
        );
        assert_eq!(out, key, "the key the instance was registered under");
        assert_eq!(
            zerodds_dw_instance_key_hash(writer, handle.wrapping_add(7), out.as_mut_ptr()),
            BAD_PARAMETER,
            "a handle this writer did not register"
        );

        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}
