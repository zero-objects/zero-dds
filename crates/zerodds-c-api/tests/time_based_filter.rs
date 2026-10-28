//! TIME_BASED_FILTER through the C API (DDS 1.4 §2.2.3.12): a C reader
//! gets one sample per instance and `minimum_separation`, and under
//! RELIABLE the last of the filtered ones once the writer went quiet: "In
//! the case where the reliability QoS kind is RELIABLE then in
//! steady-state ... the system should guarantee delivery the last sample
//! to the DataReader." The filter dropped it.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::ffi::CString;
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::entities::ZeroDdsDataReader;
use zerodds::extra_ffi::zerodds_sub_get_default_datareader_qos;
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
    zerodds_dp_delete_publisher, zerodds_dp_delete_subscriber, zerodds_dp_delete_topic,
};
use zerodds::publisher_ffi::{
    zerodds_dw_wait_for_matched, zerodds_dw_write, zerodds_pub_create_datawriter,
    zerodds_pub_delete_datawriter,
};
use zerodds::qos_ffi::{ZeroDdsDataReaderQos, ZeroDdsDuration};
use zerodds::subscriber_ffi::{
    ZeroDdsSampleArray, zerodds_dr_return_loan, zerodds_dr_take, zerodds_sub_create_datareader,
    zerodds_sub_delete_datareader,
};

const OK: i32 = 0;
const BEST_EFFORT: u32 = 1;
const RELIABLE: u32 = 2;
const KEEP_ALL: u32 = 1;

/// The payloads one take of `dr` returns.
fn take(dr: *mut ZeroDdsDataReader) -> Vec<Vec<u8>> {
    let mut arr = ZeroDdsSampleArray {
        buffers: ptr::null_mut(),
        lengths: ptr::null_mut(),
        infos: ptr::null_mut(),
        count: 0,
        loan_token: ptr::null_mut(),
    };
    // SAFETY: the reader is live; `arr` is a valid out-parameter, and its
    // arrays hold `count` entries until the loan is returned.
    unsafe {
        let _ = zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0);
        let mut out = Vec::new();
        for i in 0..arr.count {
            if (*arr.infos.add(i)).valid_data {
                let buf = *arr.buffers.add(i);
                let len = *arr.lengths.add(i);
                out.push(std::slice::from_raw_parts(buf, len).to_vec());
            }
        }
        if arr.count > 0 {
            assert_eq!(zerodds_dr_return_loan(dr, &mut arr), OK);
        }
        out
    }
}

/// What takes of `dr` return until `want` samples arrived or `within`
/// passed.
fn take_until(dr: *mut ZeroDdsDataReader, want: usize, within: Duration) -> Vec<Vec<u8>> {
    let deadline = Instant::now() + within;
    let mut got = Vec::new();
    loop {
        got.extend(take(dr));
        if got.len() >= want || Instant::now() >= deadline {
            return got;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Writes `[1]`, `[2]`, `[3]` back to back to a same-participant reader
/// with a TIME_BASED_FILTER of 300 ms and `reliability`, and returns what
/// it took at once and what it took after.
fn three_samples_at_once(reliability: u32, topic: &str) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic_name = CString::new(topic).unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    // SAFETY: every pointer below is NULL-checked before use, per each
    // callee's own `# Safety` contract; the CStrings live to the end of the
    // block.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
        assert!(!dp.is_null(), "participant create failed");
        let t = zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        assert!(!t.is_null() && !publisher.is_null() && !subscriber.is_null());
        let writer = zerodds_pub_create_datawriter(publisher, t, ptr::null());
        let mut dr_qos: ZeroDdsDataReaderQos = std::mem::zeroed();
        assert_eq!(
            zerodds_sub_get_default_datareader_qos(subscriber, &mut dr_qos),
            OK
        );
        dr_qos.reliability.kind = reliability;
        dr_qos.history.kind = KEEP_ALL;
        dr_qos.time_based_filter.minimum_separation = ZeroDdsDuration {
            sec: 0,
            nanosec: 300_000_000,
        };
        let reader = zerodds_sub_create_datareader(subscriber, t, &dr_qos);
        assert!(!writer.is_null() && !reader.is_null());
        assert_eq!(zerodds_dw_wait_for_matched(writer, 1, 5_000), OK);
        for payload in [[1u8], [2], [3]] {
            assert_eq!(zerodds_dw_write(writer, payload.as_ptr(), 1, 0), OK);
        }
        let first = take_until(reader, 1, Duration::from_secs(2));
        let after = take_until(reader, 2, Duration::from_millis(1_000));
        zerodds_sub_delete_datareader(subscriber, reader);
        zerodds_pub_delete_datawriter(publisher, writer);
        zerodds_dp_delete_subscriber(dp, subscriber);
        zerodds_dp_delete_publisher(dp, publisher);
        zerodds_dp_delete_topic(dp, t);
        zerodds_dpf_delete_participant(f, dp);
        (first, after)
    }
}

#[test]
fn a_reliable_c_reader_gets_the_last_filtered_sample_once_the_writer_is_quiet() {
    let (first, after) = three_samples_at_once(RELIABLE, "CTbfLastSample");
    assert_eq!(first, vec![vec![1u8]]);
    assert_eq!(
        after,
        vec![vec![3u8]],
        "the last sample, not the one between"
    );
}

#[test]
fn a_best_effort_c_reader_gets_no_filtered_sample() {
    let (first, after) = three_samples_at_once(BEST_EFFORT, "CTbfLastSampleBe");
    assert_eq!(first, vec![vec![1u8]]);
    assert!(after.is_empty(), "{after:?}");
}
