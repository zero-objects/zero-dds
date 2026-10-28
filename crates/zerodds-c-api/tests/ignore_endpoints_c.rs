// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds_dp_ignore_publication` and `zerodds_dp_ignore_subscription` (DDS
//! 1.4 §2.2.2.2.1.16, §2.2.2.2.1.17) end the match that exists between two
//! participants of one process, with the handle `get_matched_publications` /
//! `get_matched_subscriptions` returned.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ptr;
use std::time::{Duration, Instant};

use zerodds::extra_ffi::{
    zerodds_dr_get_matched_publications, zerodds_dw_get_matched_subscriptions,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
    zerodds_dp_ignore_publication, zerodds_dp_ignore_subscription,
};
use zerodds::publisher_ffi::zerodds_pub_create_datawriter;
use zerodds::subscriber_ffi::zerodds_sub_create_datareader;

fn eventually(cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

#[test]
fn ignore_publication_and_subscription_end_the_match_of_two_participants() {
    let lease = zerodds_test_domains::lease_domain();
    // SAFETY: handles come from the create calls above their use and are
    // released at the end.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let a = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        let b = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        assert!(!a.is_null() && !b.is_null());
        let ta =
            zerodds_dp_create_topic(a, c"IgnoreC".as_ptr(), c"IgnoreCType".as_ptr(), ptr::null());
        let tb =
            zerodds_dp_create_topic(b, c"IgnoreC".as_ptr(), c"IgnoreCType".as_ptr(), ptr::null());
        let writer = zerodds_pub_create_datawriter(
            zerodds_dp_create_publisher(a, ptr::null()),
            ta,
            ptr::null(),
        );
        let reader = zerodds_sub_create_datareader(
            zerodds_dp_create_subscriber(b, ptr::null()),
            tb,
            ptr::null(),
        );
        assert!(!writer.is_null() && !reader.is_null());

        let matched_publications = || {
            let mut handles = [0u64; 8];
            let mut count = 0usize;
            assert_eq!(
                zerodds_dr_get_matched_publications(reader, handles.as_mut_ptr(), &mut count, 8),
                0
            );
            handles[..count.min(8)].to_vec()
        };
        let matched_subscriptions = || {
            let mut handles = [0u64; 8];
            let mut count = 0usize;
            assert_eq!(
                zerodds_dw_get_matched_subscriptions(writer, handles.as_mut_ptr(), &mut count, 8),
                0
            );
            handles[..count.min(8)].to_vec()
        };
        assert!(eventually(
            || !matched_publications().is_empty() && !matched_subscriptions().is_empty()
        ));

        let publication = matched_publications()[0];
        assert_eq!(zerodds_dp_ignore_publication(b, publication), 0);
        assert!(
            eventually(|| matched_publications().is_empty()),
            "the ignored publication stayed matched"
        );

        let subscription = matched_subscriptions().first().copied();
        if let Some(subscription) = subscription {
            assert_eq!(zerodds_dp_ignore_subscription(a, subscription), 0);
        }
        assert!(
            eventually(|| matched_subscriptions().is_empty()),
            "the ignored subscription stayed matched"
        );

        let _ = zerodds_dpf_delete_participant(f, a);
        let _ = zerodds_dpf_delete_participant(f, b);
    }
}
