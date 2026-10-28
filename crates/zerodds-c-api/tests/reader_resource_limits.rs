//! HISTORY and RESOURCE_LIMITS of a C reader (DDS 1.4 §2.2.3.18,
//! §2.2.3.19, §2.2.4.1): over `max_samples_per_instance` a RELIABLE reader
//! refuses the sample, counts SAMPLE_REJECTED with
//! REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT, and delivers it once there is
//! room: "a rejected sample may be accepted in the future". The C reader
//! applied only `max_instances`.

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
use zerodds::qos_ffi::ZeroDdsDataReaderQos;
use zerodds::subscriber_ffi::{
    ZeroDdsSampleArray, ZeroDdsSampleRejectedStatus, zerodds_dr_get_sample_rejected_status,
    zerodds_dr_return_loan, zerodds_dr_take, zerodds_sub_create_datareader,
    zerodds_sub_delete_datareader,
};

const OK: i32 = 0;
const RELIABLE: u32 = 2;
const KEEP_ALL: u32 = 1;
const BY_SAMPLES_PER_INSTANCE_LIMIT: u32 = 3;

/// How many samples one take of `dr` returns.
fn take_count(dr: *mut ZeroDdsDataReader) -> usize {
    let mut arr = ZeroDdsSampleArray {
        buffers: ptr::null_mut(),
        lengths: ptr::null_mut(),
        infos: ptr::null_mut(),
        count: 0,
        loan_token: ptr::null_mut(),
    };
    // SAFETY: the reader is live; `arr` is a valid out-parameter.
    unsafe {
        let _ = zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0);
        let n = arr.count;
        if n > 0 {
            assert_eq!(zerodds_dr_return_loan(dr, &mut arr), OK);
        }
        n
    }
}

#[test]
fn a_reliable_c_reader_over_its_instance_limit_rejects_and_delivers_later() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic_name = CString::new("CReaderSamplesPerInstance").unwrap();
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
        dr_qos.reliability.kind = RELIABLE;
        dr_qos.history.kind = KEEP_ALL;
        dr_qos.resource_limits.max_samples_per_instance = 2;
        let reader = zerodds_sub_create_datareader(subscriber, t, &dr_qos);
        assert!(!writer.is_null() && !reader.is_null());
        assert_eq!(zerodds_dw_wait_for_matched(writer, 1, 5_000), OK);

        // An untyped keyed reader keys by the payload: one instance.
        let payload = [5u8];
        for _ in 0..3 {
            assert_eq!(zerodds_dw_write(writer, payload.as_ptr(), 1, 0), OK);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut first = 0;
        while first == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
            first = take_count(reader);
        }
        assert_eq!(first, 2, "two fit the instance");
        let mut status: ZeroDdsSampleRejectedStatus = std::mem::zeroed();
        assert_eq!(
            zerodds_dr_get_sample_rejected_status(reader, &mut status),
            OK
        );
        assert_eq!(status.total_count, 1);
        assert_eq!(status.last_reason, BY_SAMPLES_PER_INSTANCE_LIMIT);
        assert_eq!(
            take_count(reader),
            1,
            "the refused one comes with the next take"
        );

        zerodds_sub_delete_datareader(subscriber, reader);
        zerodds_pub_delete_datawriter(publisher, writer);
        zerodds_dp_delete_subscriber(dp, subscriber);
        zerodds_dp_delete_publisher(dp, publisher);
        zerodds_dp_delete_topic(dp, t);
        zerodds_dpf_delete_participant(f, dp);
    }
}
