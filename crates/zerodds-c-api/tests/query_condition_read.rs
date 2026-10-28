// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `read_w_condition` and `take_w_condition` apply the expression of a QueryCondition (DDS 1.4
//! §2.2.2.5.9): they returned every sample the state masks let through.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod typed_writer;

use std::ffi::{CString, c_void};
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::condition_ffi::{
    zerodds_dr_create_querycondition, zerodds_dr_delete_readcondition,
    zerodds_querycondition_set_expression, zerodds_querycondition_set_parameters,
};
use zerodds::dyntype_ffi::{
    zerodds_dyntype_bytes_free, zerodds_dyntype_destroy, zerodds_dyntype_encode_json,
    zerodds_dyntype_extensibility, zerodds_dyntype_is_keyed,
};
use zerodds::extra_ffi::{
    zerodds_dr_read_w_condition, zerodds_dr_set_type_objects,
    zerodds_dr_take_next_instance_w_condition, zerodds_dr_take_w_condition,
    zerodds_sub_get_default_datareader_qos,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
};
use zerodds::publisher_ffi::zerodds_dw_write;
use zerodds::qos_ffi::ZeroDdsDataReaderQos;
use zerodds::subscriber_ffi::{
    ZeroDdsSampleArray, zerodds_dr_return_loan, zerodds_sub_create_datareader_typed,
};

const OK: i32 = 0;
const PRECONDITION_NOT_MET: i32 = -5;
const NO_DATA: i32 = -7;

#[test]
fn a_query_condition_without_a_type_has_no_data_to_judge_when_empty_and_refuses_a_sample_it_cannot_read()
 {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("CQueryNoType").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    let expr = CString::new("x > %0").unwrap();
    let param = CString::new("1").unwrap();
    // SAFETY: handles come from the create calls above their use; the participant is deleted
    // once at the end.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let sub = zerodds_dp_create_subscriber(dp, ptr::null());
        let reader =
            zerodds::subscriber_ffi::zerodds_sub_create_datareader(sub, topic, ptr::null());
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
        let mut arr: ZeroDdsSampleArray = core::mem::zeroed();
        // Nothing to judge: no data, not an error.
        assert_eq!(
            zerodds_dr_read_w_condition(reader, &mut arr, 8, query),
            NO_DATA
        );
        // A sample the expression cannot be evaluated on: an error, not a pass.
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let writer =
            zerodds::publisher_ffi::zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        assert_eq!(zerodds_dw_write(writer, b"abcd".as_ptr(), 4, 0), OK);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut rc = NO_DATA;
        while Instant::now() < deadline {
            rc = zerodds_dr_read_w_condition(reader, &mut arr, 8, query);
            if rc != NO_DATA {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(rc, PRECONDITION_NOT_MET);
        assert_eq!(
            zerodds_dr_take_w_condition(reader, &mut arr, 8, query),
            PRECONDITION_NOT_MET
        );
        let _ = zerodds_dr_delete_readcondition(reader, query);
        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}

/// A reader that keeps every sample: a topic without a key has one instance
/// (DDS 1.4 §2.2.1.2.2), and the default KEEP_LAST(1) (§2.2.3.18) would keep
/// only the last sample of it.
///
/// # Safety
/// `sub` is a live subscriber.
unsafe fn keep_all_qos(sub: *mut zerodds::entities::ZeroDdsSubscriber) -> ZeroDdsDataReaderQos {
    // SAFETY: the caller's contract; the struct is plain data.
    unsafe {
        let mut qos: ZeroDdsDataReaderQos = core::mem::zeroed();
        assert_eq!(zerodds_sub_get_default_datareader_qos(sub, &mut qos), OK);
        qos.history.kind = 1;
        qos
    }
}

/// Reads with `query` until at least `want` samples arrive or five seconds pass; returns the
/// `x` of each sample, in order.
///
/// # Safety
/// `reader` and `query` are live handles of the same reader.
unsafe fn read_xs(
    reader: *mut zerodds::entities::ZeroDdsDataReader,
    query: *mut c_void,
    take: bool,
) -> Vec<i32> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // SAFETY: the caller's handles are live; `arr` is returned before the next round.
        unsafe {
            let mut arr: ZeroDdsSampleArray = core::mem::zeroed();
            let rc = if take {
                zerodds_dr_take_w_condition(reader, &mut arr, 16, query)
            } else {
                zerodds_dr_read_w_condition(reader, &mut arr, 16, query)
            };
            if rc == OK && arr.count > 0 {
                let mut xs = Vec::new();
                for i in 0..arr.count {
                    let len = *arr.lengths.add(i);
                    let buf = std::slice::from_raw_parts(*arr.buffers.add(i), len);
                    // XCDR1 little endian (the representation of a writer created without a wire
                    // contract): the `long x` is the first four octets.
                    xs.push(i32::from_le_bytes(buf[0..4].try_into().unwrap()));
                }
                zerodds_dr_return_loan(reader, &mut arr);
                return xs;
            }
            if Instant::now() > deadline {
                return Vec::new();
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn read_and_take_with_a_query_condition_return_only_the_samples_the_expression_selects() {
    let lease = zerodds_test_domains::lease_domain();
    let expr = CString::new("x > %0").unwrap();
    let below = CString::new("x < %0").unwrap();
    let four = CString::new("4").unwrap();
    // SAFETY: handles come from the create calls above their use and are released at the end;
    // the TypeObject pointer stays valid until `zerodds_dyntype_destroy`.
    unsafe {
        let typed_writer::TypedWriter {
            t,
            object,
            object_len,
            f,
            dp,
            topic,
            writer,
        } = typed_writer::typed_writer(
            "struct QRow { long x; };",
            "QRow",
            "CQueryRead",
            lease.id() as u32,
        );
        let sub = zerodds_dp_create_subscriber(dp, ptr::null());
        let reader_qos = keep_all_qos(sub);
        let reader =
            zerodds_sub_create_datareader_typed(sub, topic, &reader_qos, object, object_len);
        assert!(!reader.is_null());
        assert_eq!(
            zerodds_dr_set_type_objects(
                reader,
                object,
                object_len,
                ptr::null(),
                ptr::null(),
                0,
                zerodds_dyntype_is_keyed(t),
                zerodds_dyntype_extensibility(t) as u32,
            ),
            OK
        );

        for x in [1, 5, 9] {
            let value = CString::new(format!("{{\"x\":{x}}}")).unwrap();
            let mut bytes = ptr::null_mut();
            let mut size = 0usize;
            assert_eq!(
                zerodds_dyntype_encode_json(t, value.as_ptr(), 0, 0, &mut bytes, &mut size),
                OK
            );
            assert_eq!(zerodds_dw_write(writer, bytes, size, 0), OK);
            zerodds_dyntype_bytes_free(bytes, size);
        }

        let params = [four.as_ptr()];
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

        // x > 4: 5 and 9, not 1. A read leaves them in the reader.
        let mut greater = read_xs(reader, query, false);
        greater.sort_unstable();
        assert_eq!(greater, vec![5, 9]);

        // The same condition with another expression selects the other sample.
        assert_eq!(
            zerodds_querycondition_set_expression(query, below.as_ptr()),
            OK
        );
        assert_eq!(
            zerodds_querycondition_set_parameters(query, params.as_ptr(), 1),
            OK
        );
        assert_eq!(read_xs(reader, query, false), vec![1]);

        // take removes what the expression selected and nothing else.
        assert_eq!(
            zerodds_querycondition_set_expression(query, expr.as_ptr()),
            OK
        );
        let mut taken = read_xs(reader, query, true);
        taken.sort_unstable();
        assert_eq!(taken, vec![5, 9]);
        assert_eq!(
            zerodds_querycondition_set_expression(query, below.as_ptr()),
            OK
        );
        assert_eq!(read_xs(reader, query, true), vec![1]);

        let _ = zerodds_dr_delete_readcondition(reader, query);
        zerodds_dyntype_destroy(t);
        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}

/// DDS 1.4 §2.2.2.5.3.12 `take_next_instance_w_condition`: the samples of the next instance
/// after `previous_handle` that the condition selects, its expression included. A keyless
/// type has one instance (§2.2.1.2.2): the first call takes the samples the expression
/// selects, the one after the handle it returned finds no further instance.
#[test]
fn take_next_instance_w_condition_applies_the_expression_to_the_one_instance_of_a_keyless_type() {
    let lease = zerodds_test_domains::lease_domain();
    let expr = CString::new("x > %0").unwrap();
    let four = CString::new("4").unwrap();
    // SAFETY: as in the test above.
    unsafe {
        let typed_writer::TypedWriter {
            t,
            object,
            object_len,
            f,
            dp,
            topic,
            writer,
        } = typed_writer::typed_writer(
            "struct QRow { long x; };",
            "QRow",
            "CQueryNextInstance",
            lease.id() as u32,
        );
        let sub = zerodds_dp_create_subscriber(dp, ptr::null());
        let reader_qos = keep_all_qos(sub);
        let reader =
            zerodds_sub_create_datareader_typed(sub, topic, &reader_qos, object, object_len);
        assert!(!writer.is_null() && !reader.is_null());
        assert_eq!(
            zerodds_dr_set_type_objects(
                reader,
                object,
                object_len,
                ptr::null(),
                ptr::null(),
                0,
                zerodds_dyntype_is_keyed(t),
                zerodds_dyntype_extensibility(t) as u32,
            ),
            OK
        );
        for x in [1, 5, 9] {
            let value = CString::new(format!("{{\"x\":{x}}}")).unwrap();
            let mut bytes = ptr::null_mut();
            let mut size = 0usize;
            assert_eq!(
                zerodds_dyntype_encode_json(t, value.as_ptr(), 0, 0, &mut bytes, &mut size),
                OK
            );
            assert_eq!(zerodds_dw_write(writer, bytes, size, 0), OK);
            zerodds_dyntype_bytes_free(bytes, size);
        }
        let params = [four.as_ptr()];
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

        // Wait until both samples the expression selects are there: a take
        // that came between the two arrivals would take one of them away.
        let deadline = Instant::now() + Duration::from_secs(5);
        while read_xs(reader, query, false).len() < 2 {
            assert!(Instant::now() < deadline, "no sample arrived");
            std::thread::sleep(Duration::from_millis(50));
        }
        let mut arr: ZeroDdsSampleArray = core::mem::zeroed();
        assert_eq!(
            zerodds_dr_take_next_instance_w_condition(reader, 0, &mut arr, 16, query),
            OK
        );
        let (count, handle) = (arr.count, (*arr.infos).instance_handle);
        zerodds_dr_return_loan(reader, &mut arr);
        assert_eq!(count, 2, "x > 4 selects 5 and 9, not 1");
        assert_ne!(handle, 0, "the one instance has a handle");

        let mut arr: ZeroDdsSampleArray = core::mem::zeroed();
        assert_eq!(
            zerodds_dr_take_next_instance_w_condition(reader, handle, &mut arr, 16, query),
            NO_DATA,
            "no instance after the only one"
        );

        let _ = zerodds_dr_delete_readcondition(reader, query);
        zerodds_dyntype_destroy(t);
        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}

/// Annex B.2 `QueryExpression`: `ORDER BY` orders what a read or take returns,
/// ascending, whatever the order the samples arrived in; a condition before it
/// still selects.
#[test]
fn order_by_orders_what_read_and_take_return() {
    let lease = zerodds_test_domains::lease_domain();
    let ordered = CString::new("x > %0 ORDER BY x").unwrap();
    let zero = CString::new("0").unwrap();
    // SAFETY: as in the first test of this file.
    unsafe {
        let typed_writer::TypedWriter {
            t,
            object,
            object_len,
            f,
            dp,
            topic,
            writer,
        } = typed_writer::typed_writer(
            "struct QOrdered { long x; };",
            "QOrdered",
            "CQueryOrderBy",
            lease.id() as u32,
        );
        let sub = zerodds_dp_create_subscriber(dp, ptr::null());
        let reader_qos = keep_all_qos(sub);
        let reader =
            zerodds_sub_create_datareader_typed(sub, topic, &reader_qos, object, object_len);
        assert!(!writer.is_null() && !reader.is_null());
        assert_eq!(
            zerodds_dr_set_type_objects(
                reader,
                object,
                object_len,
                ptr::null(),
                ptr::null(),
                0,
                zerodds_dyntype_is_keyed(t),
                zerodds_dyntype_extensibility(t) as u32,
            ),
            OK
        );
        for x in [9, -3, 5, 1] {
            let value = CString::new(format!("{{\"x\":{x}}}")).unwrap();
            let mut bytes = ptr::null_mut();
            let mut size = 0usize;
            assert_eq!(
                zerodds_dyntype_encode_json(t, value.as_ptr(), 0, 0, &mut bytes, &mut size),
                OK
            );
            assert_eq!(zerodds_dw_write(writer, bytes, size, 0), OK);
            zerodds_dyntype_bytes_free(bytes, size);
        }
        let params = [zero.as_ptr()];
        let query = zerodds_dr_create_querycondition(
            reader,
            0xFFFF,
            0xFFFF,
            0xFFFF,
            ordered.as_ptr(),
            params.as_ptr(),
            1,
        ) as *mut c_void;
        assert!(!query.is_null());

        // Wait for all four, then read: x > 0 selects 9, 5, 1, ORDER BY x sorts them.
        let deadline = Instant::now() + Duration::from_secs(5);
        let read = loop {
            let xs = read_xs(reader, query, false);
            if xs.len() >= 3 || Instant::now() > deadline {
                break xs;
            }
        };
        assert_eq!(read, vec![1, 5, 9]);
        assert_eq!(read_xs(reader, query, true), vec![1, 5, 9]);

        let _ = zerodds_dr_delete_readcondition(reader, query);
        zerodds_dyntype_destroy(t);
        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}
