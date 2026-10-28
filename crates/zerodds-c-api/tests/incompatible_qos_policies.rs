// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds_dw_get_offered_incompatible_qos_status_ex` and
//! `zerodds_dr_get_requested_incompatible_qos_status_ex` carry the per-policy
//! counts of the incompatible-QoS statuses (DDS 1.4 §2.2.4.1
//! `QosPolicyCountSeq policies`; DDS-PSM-Cxx 1.0 §7.5.4), where the plain
//! status has the last policy id only.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::CString;
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::extra_ffi::{
    zerodds_pub_get_default_datawriter_qos, zerodds_sub_get_default_datareader_qos,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
};
use zerodds::publisher_ffi::zerodds_pub_create_datawriter;
use zerodds::qos_ffi::{ZeroDdsDataReaderQos, ZeroDdsDataWriterQos};
use zerodds::status_ffi::{
    ZeroDdsIncompatibleQosStatusEx, zerodds_dr_get_requested_incompatible_qos_status_ex,
    zerodds_dw_get_offered_incompatible_qos_status_ex,
};
use zerodds::subscriber_ffi::zerodds_sub_create_datareader;
use zerodds_dcps::psm_constants::qos_policy_id;

const OK: i32 = 0;
const BEST_EFFORT: u32 = 1;
const RELIABLE: u32 = 2;

#[test]
fn both_sides_report_the_incompatible_policy_with_its_count() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CIncompatibleQosPolicies").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    // SAFETY: every pointer comes from the create call above its use and is
    // checked; the participant is deleted once at the end.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        assert!(!dp.is_null());
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        let mut wq: ZeroDdsDataWriterQos = std::mem::zeroed();
        assert_eq!(
            zerodds_pub_get_default_datawriter_qos(publisher, &mut wq),
            OK
        );
        wq.reliability.kind = BEST_EFFORT;
        let mut rq: ZeroDdsDataReaderQos = std::mem::zeroed();
        assert_eq!(
            zerodds_sub_get_default_datareader_qos(subscriber, &mut rq),
            OK
        );
        rq.reliability.kind = RELIABLE;
        let writer = zerodds_pub_create_datawriter(publisher, topic, &wq);
        let reader = zerodds_sub_create_datareader(subscriber, topic, &rq);
        assert!(!writer.is_null() && !reader.is_null());

        // The offered side learns of the reader's request by discovery.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut offered = ZeroDdsIncompatibleQosStatusEx::default();
        loop {
            assert_eq!(
                zerodds_dw_get_offered_incompatible_qos_status_ex(writer, &mut offered),
                OK
            );
            if offered.total_count > 0 || Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(offered.total_count >= 1, "no incompatibility was detected");
        assert_eq!(offered.last_policy_id, qos_policy_id::RELIABILITY);
        assert_eq!(offered.policies_len, 1);
        assert_eq!(offered.policies[0].policy_id, qos_policy_id::RELIABILITY);
        assert_eq!(offered.policies[0].count, offered.total_count);

        let mut requested = ZeroDdsIncompatibleQosStatusEx::default();
        loop {
            assert_eq!(
                zerodds_dr_get_requested_incompatible_qos_status_ex(reader, &mut requested),
                OK
            );
            if requested.total_count > 0 || Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(requested.total_count >= 1);
        assert_eq!(requested.policies_len, 1);
        assert_eq!(requested.policies[0].policy_id, qos_policy_id::RELIABILITY);

        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}
