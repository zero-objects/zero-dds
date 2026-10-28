// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds_querycondition_set_expression` and `_set_parameters` change a
//! query condition (DDS-PSM-Cxx 1.0 `QueryCondition::expression(str)`, DDS 1.4
//! §2.2.2.5.9 `set_query_parameters`); the `*_next_instance_w_condition`
//! reads take the state masks of a condition.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::{CString, c_void};
use std::ptr;

use zerodds::condition_ffi::{
    zerodds_dr_create_querycondition, zerodds_dr_create_readcondition,
    zerodds_dr_delete_readcondition, zerodds_querycondition_set_expression,
    zerodds_querycondition_set_parameters,
};
use zerodds::extra_ffi::{
    zerodds_dr_read_next_instance_w_condition, zerodds_dr_take_next_instance_w_condition,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{zerodds_dp_create_subscriber, zerodds_dp_create_topic};
use zerodds::subscriber_ffi::{ZeroDdsSampleArray, zerodds_sub_create_datareader};

const OK: i32 = 0;
const BAD_HANDLE: i32 = -2;
const BAD_PARAMETER: i32 = -6;
const NO_DATA: i32 = -7;

#[test]
fn a_query_condition_takes_a_new_expression_and_new_parameters() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CQueryUpdate").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    let expr = CString::new("x > %0").unwrap();
    let next_expr = CString::new("x < %0").unwrap();
    let param = CString::new("1").unwrap();
    let next_param = CString::new("2").unwrap();
    // SAFETY: every pointer comes from the create call above its use and is
    // checked; the participant is deleted once at the end.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let sub = zerodds_dp_create_subscriber(dp, ptr::null());
        let reader = zerodds_sub_create_datareader(sub, topic, ptr::null());
        assert!(!reader.is_null());

        let params = [param.as_ptr()];
        let query = zerodds_dr_create_querycondition(
            reader,
            0xFFFF,
            0xFFFF,
            0xFFFF,
            expr.as_ptr(),
            params.as_ptr(),
            1,
        ) as *mut c_void;
        assert!(!query.is_null());
        assert_eq!(
            zerodds_querycondition_set_expression(query, next_expr.as_ptr()),
            OK
        );
        let next_params = [next_param.as_ptr()];
        assert_eq!(
            zerodds_querycondition_set_parameters(query, next_params.as_ptr(), 1),
            OK
        );
        assert_eq!(
            zerodds_querycondition_set_parameters(query, ptr::null(), 0),
            OK
        );
        assert_eq!(
            zerodds_querycondition_set_parameters(query, ptr::null(), 1),
            BAD_PARAMETER,
            "a count without a pointer"
        );
        assert_eq!(
            zerodds_querycondition_set_expression(query, ptr::null()),
            BAD_PARAMETER
        );

        // A ReadCondition is not a query condition.
        let read = zerodds_dr_create_readcondition(reader, 0xFFFF, 0xFFFF, 0xFFFF) as *mut c_void;
        assert_eq!(
            zerodds_querycondition_set_expression(read, next_expr.as_ptr()),
            BAD_HANDLE
        );
        assert_eq!(
            zerodds_querycondition_set_expression(ptr::null_mut(), next_expr.as_ptr()),
            BAD_HANDLE
        );

        // Nothing was written: the next-instance reads find no data, and a
        // pointer that is no condition is refused.
        let mut arr: ZeroDdsSampleArray = core::mem::zeroed();
        let rc = zerodds_dr_read_next_instance_w_condition(reader, 0, &mut arr, 8, query);
        assert!(
            rc == NO_DATA || rc == OK,
            "read_next_instance_w_condition: {rc}"
        );
        let rc = zerodds_dr_take_next_instance_w_condition(reader, 0, &mut arr, 8, read);
        assert!(
            rc == NO_DATA || rc == OK,
            "take_next_instance_w_condition: {rc}"
        );
        assert_eq!(
            zerodds_dr_take_next_instance_w_condition(reader, 0, &mut arr, 8, ptr::null_mut()),
            BAD_PARAMETER
        );

        let _ = zerodds_dr_delete_readcondition(reader, query);
        let _ = zerodds_dr_delete_readcondition(reader, read);
        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}
