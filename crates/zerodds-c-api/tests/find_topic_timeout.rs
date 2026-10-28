// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds_dp_find_topic_w_timeout` (DDS 1.4 §2.2.2.2.1.11): the topic if
//! there is one, otherwise it waits for another thread to create it, and a
//! timeout returns nil.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::CString;
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{zerodds_dp_create_topic, zerodds_dp_find_topic_w_timeout};

#[test]
fn find_topic_waits_for_a_topic_and_times_out_without_one() {
    let lease = zerodds_test_domains::lease_domain();
    let name = CString::new("CFindTimeout").unwrap();
    let type_name = CString::new("FindTimeoutType").unwrap();
    // SAFETY: handles come from the create calls above their use; the
    // participant outlives the thread, which is joined before the delete.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        assert!(!dp.is_null());

        let started = Instant::now();
        assert!(zerodds_dp_find_topic_w_timeout(dp, name.as_ptr(), 100).is_null());
        assert!(started.elapsed() >= Duration::from_millis(100));

        let address = dp as usize;
        let (n, t) = (name.clone(), type_name.clone());
        let creator = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            // The participant is alive until the join below.
            let topic =
                zerodds_dp_create_topic(address as *mut _, n.as_ptr(), t.as_ptr(), ptr::null());
            topic as usize
        });
        let found = zerodds_dp_find_topic_w_timeout(dp, name.as_ptr(), 5_000);
        let created = creator.join().unwrap();
        assert!(!found.is_null(), "the topic created meanwhile was found");
        assert_eq!(found as usize, created);

        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}
