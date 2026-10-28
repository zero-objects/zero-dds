// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MultiTopic through the C API (DDS 1.4 §2.2.2.3.4, Annex B.2): a participant with the topics
//! `Order` and `Customer` and a MultiTopic `Sale` over `SELECT * FROM Order NATURAL JOIN
//! Customer`; a reader on it gets the joined samples; `delete_multitopic` is refused while the
//! reader exists.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::{CStr, CString};
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::dyntype_ffi::{
    ZeroDdsDynType, zerodds_dyntype_bytes_free, zerodds_dyntype_decode_json,
    zerodds_dyntype_dependency, zerodds_dyntype_dependency_count, zerodds_dyntype_destroy,
    zerodds_dyntype_encode_json, zerodds_dyntype_extensibility, zerodds_dyntype_from_idl,
    zerodds_dyntype_is_keyed, zerodds_dyntype_type_object,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::multitopic_ffi::{
    ZeroDdsMultiTopicSample, zerodds_dp_create_multitopic, zerodds_dp_delete_multitopic,
    zerodds_multitopic_reader_destroy, zerodds_multitopic_reader_read,
    zerodds_multitopic_samples_free, zerodds_multitopic_set_parameters,
    zerodds_sub_create_multitopic_reader, zerodds_topic_set_type_objects,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
};
use zerodds::publisher_ffi::{zerodds_dw_write, zerodds_pub_create_datawriter_typed_wire};
use zerodds::topic_ffi::zerodds_string_free;

const OK: i32 = 0;
const BAD_PARAMETER: i32 = -6;
const PRECONDITION_NOT_MET: i32 = -5;

/// A dynamic type of `name` in `idl`.
///
/// # Safety
/// Calls the C API with NUL-terminated strings it builds itself.
unsafe fn dyntype(idl: &str, name: &str) -> *mut ZeroDdsDynType {
    let idl = CString::new(idl).unwrap();
    let name = CString::new(name).unwrap();
    let mut status = -1;
    // SAFETY: NUL-terminated strings.
    let t = unsafe { zerodds_dyntype_from_idl(idl.as_ptr(), name.as_ptr(), &mut status) };
    assert!(!t.is_null(), "status {status}");
    t
}

/// The type object and dependencies of `t`, in the arrays the C functions take.
///
/// # Safety
/// `t` is a live dynamic type.
unsafe fn objects(t: *const ZeroDdsDynType) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut len = 0usize;
    // SAFETY: `t` is live; `len` is the length of the object it returns.
    let pointer = unsafe { zerodds_dyntype_type_object(t, &mut len) };
    // SAFETY: `pointer[0..len]` is the object just returned.
    let object = unsafe { std::slice::from_raw_parts(pointer, len) }.to_vec();
    let mut deps = Vec::new();
    // SAFETY: `t` is live.
    for i in 0..unsafe { zerodds_dyntype_dependency_count(t) } {
        let mut n = 0usize;
        // SAFETY: as above.
        let p = unsafe { zerodds_dyntype_dependency(t, i, &mut n) };
        // SAFETY: `p[0..n]` readable.
        deps.push(unsafe { std::slice::from_raw_parts(p, n) }.to_vec());
    }
    (object, deps)
}

/// The XCDR2 body of the JSON value.
///
/// # Safety
/// `t` is a live dynamic type.
unsafe fn encode(t: *const ZeroDdsDynType, json: &str) -> Vec<u8> {
    let json = CString::new(json).unwrap();
    let mut bytes = ptr::null_mut();
    let mut len = 0usize;
    // SAFETY: NUL-terminated; out pointers are locals.
    assert_eq!(
        unsafe { zerodds_dyntype_encode_json(t, json.as_ptr(), 1, 0, &mut bytes, &mut len) },
        OK
    );
    // SAFETY: the octets the call returned.
    let out = unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec();
    // SAFETY: released once.
    unsafe { zerodds_dyntype_bytes_free(bytes, len) };
    out
}

#[test]
fn a_multitopic_joins_two_topics_and_is_not_deleted_while_a_reader_exists() {
    let lease = zerodds_test_domains::lease_domain();
    // SAFETY: every pointer comes from the call above its use and is checked; the participant is
    // deleted once at the end.
    unsafe {
        let order_t = dyntype(
            "@final struct Order { @key long id; double amount; };",
            "Order",
        );
        let customer_t = dyntype(
            "@final struct Customer { @key long id; string<16> country; };",
            "Customer",
        );
        let sale_t = dyntype(
            "@final struct Sale { @key long id; double amount; string<16> country; };",
            "Sale",
        );

        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, lease.id() as u32, ptr::null());
        assert!(!dp.is_null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());

        let mut writers = Vec::new();
        for (t, name) in [(order_t, "Order"), (customer_t, "Customer")] {
            let cname = CString::new(name).unwrap();
            let topic = zerodds_dp_create_topic(dp, cname.as_ptr(), cname.as_ptr(), ptr::null());
            assert!(!topic.is_null());
            let (object, deps) = objects(t);
            let dep_ptrs: Vec<*const u8> = deps.iter().map(|d| d.as_ptr()).collect();
            let dep_lens: Vec<usize> = deps.iter().map(Vec::len).collect();
            assert_eq!(
                zerodds_topic_set_type_objects(
                    topic,
                    object.as_ptr(),
                    object.len(),
                    if deps.is_empty() {
                        ptr::null()
                    } else {
                        dep_ptrs.as_ptr()
                    },
                    if deps.is_empty() {
                        ptr::null()
                    } else {
                        dep_lens.as_ptr()
                    },
                    deps.len(),
                    zerodds_dyntype_is_keyed(t),
                    zerodds_dyntype_extensibility(t) as u32,
                ),
                OK
            );
            let writer = zerodds_pub_create_datawriter_typed_wire(
                publisher,
                topic,
                ptr::null(),
                object.as_ptr(),
                object.len(),
                1,
                0,
                2,
            );
            assert!(!writer.is_null());
            writers.push(writer);
        }

        // An expression that does not fit the topics is refused with BAD_PARAMETER.
        let (sale_object, sale_deps) = objects(sale_t);
        let sale_dep_ptrs: Vec<*const u8> = sale_deps.iter().map(|d| d.as_ptr()).collect();
        let sale_dep_lens: Vec<usize> = sale_deps.iter().map(Vec::len).collect();
        let initial = CString::new("0").unwrap();
        let initial_params = [initial.as_ptr()];
        let create = |expression: &str, status: &mut i32| {
            let expression = CString::new(expression).unwrap();
            let name = CString::new("Joined").unwrap();
            let type_name = CString::new("Sale").unwrap();
            // The strings and arrays outlive the call.
            {
                zerodds_dp_create_multitopic(
                    dp,
                    name.as_ptr(),
                    type_name.as_ptr(),
                    expression.as_ptr(),
                    initial_params.as_ptr(),
                    1,
                    sale_object.as_ptr(),
                    sale_object.len(),
                    if sale_deps.is_empty() {
                        ptr::null()
                    } else {
                        sale_dep_ptrs.as_ptr()
                    },
                    if sale_deps.is_empty() {
                        ptr::null()
                    } else {
                        sale_dep_lens.as_ptr()
                    },
                    sale_deps.len(),
                    1,
                    0,
                    status,
                )
            }
        };
        let mut status = OK;
        assert!(create("SELECT * FROM Order ORDER BY id", &mut status).is_null());
        assert_eq!(status, BAD_PARAMETER);
        let mt = create(
            "SELECT * FROM Order NATURAL JOIN Customer WHERE amount > %0",
            &mut status,
        );
        assert!(!mt.is_null(), "status {status}");
        assert_eq!(status, OK);
        let param = CString::new("10").unwrap();
        let params = [param.as_ptr()];
        assert_eq!(
            zerodds_multitopic_set_parameters(mt, params.as_ptr(), 1),
            OK
        );

        let reader = zerodds_sub_create_multitopic_reader(subscriber, mt, ptr::null());
        assert!(!reader.is_null());
        assert_eq!(
            zerodds_dp_delete_multitopic(dp, mt),
            PRECONDITION_NOT_MET,
            "a reader exists"
        );

        let order_low = encode(order_t, r#"{"id":1,"amount":5.0}"#);
        let order_high = encode(order_t, r#"{"id":2,"amount":50.5}"#);
        let customer_1 = encode(customer_t, r#"{"id":1,"country":"DE"}"#);
        let customer_2 = encode(customer_t, r#"{"id":2,"country":"FR"}"#);
        for (w, body) in [
            (writers[1], &customer_1),
            (writers[1], &customer_2),
            (writers[0], &order_low),
            (writers[0], &order_high),
        ] {
            assert_eq!(zerodds_dw_write(w, body.as_ptr(), body.len(), 0), OK);
        }

        // The sample of order 2 (amount above the parameter 10) joins with its customer.
        let until = Instant::now() + Duration::from_secs(10);
        let mut joined = Vec::new();
        while joined.is_empty() && Instant::now() < until {
            let mut samples: *mut ZeroDdsMultiTopicSample = ptr::null_mut();
            let mut count = 0usize;
            assert_eq!(
                zerodds_multitopic_reader_read(reader, 1, &mut samples, &mut count),
                OK
            );
            for i in 0..count {
                let s = *samples.add(i);
                if s.info.valid_data {
                    let mut json = ptr::null_mut();
                    assert_eq!(
                        zerodds_dyntype_decode_json(sale_t, s.data, s.len, 1, 0, &mut json),
                        OK
                    );
                    joined.push(CStr::from_ptr(json).to_str().unwrap().to_string());
                    zerodds_string_free(json);
                }
            }
            zerodds_multitopic_samples_free(samples, count);
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(joined.len(), 1, "{joined:?}");
        let value: serde_json::Value = serde_json::from_str(&joined[0]).unwrap();
        assert_eq!(value["id"], 2);
        assert_eq!(value["country"], "FR");
        assert_eq!(value["amount"], 50.5);

        zerodds_multitopic_reader_destroy(reader);
        assert_eq!(
            zerodds_dp_delete_multitopic(dp, mt),
            OK,
            "no reader any more"
        );

        for t in [order_t, customer_t, sale_t] {
            zerodds_dyntype_destroy(t);
        }
        let _ = zerodds_dpf_delete_participant(f, dp);
    }
}
