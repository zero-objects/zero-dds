//! `zerodds_dw_write_w_timestamp` stamps the sample with the caller's
//! time, and `zerodds_dr_take_next_sample` reports it in
//! `SampleInfo.source_timestamp` (DDS 1.4 §2.2.2.4.2.12, §2.2.2.5.5).
//! The write used to ignore the timestamp and the take used to report
//! zero for every sample.

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

use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
    zerodds_dp_delete_publisher, zerodds_dp_delete_subscriber, zerodds_dp_delete_topic,
};
use zerodds::publisher_ffi::{
    zerodds_dw_wait_for_matched, zerodds_dw_write_w_timestamp, zerodds_pub_create_datawriter,
    zerodds_pub_delete_datawriter,
};
use zerodds::subscriber_ffi::{
    ZeroDdsSampleInfo, zerodds_dr_take_next_sample, zerodds_sub_create_datareader,
    zerodds_sub_delete_datareader,
};

const OK: i32 = 0;
const BAD_PARAMETER: i32 = zerodds::ZeroDdsStatus::BadParameter as i32;

#[test]
fn the_callers_timestamp_reaches_the_sample_info() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic_name = CString::new("WriteWTimestampTopic").unwrap();
    let type_name = CString::new("RawBytes").unwrap();

    // SAFETY: every pointer below is NULL-checked before use, per each
    // callee's own `# Safety` contract; CStrings live to the end of the block.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
        assert!(!dp.is_null(), "participant create failed");
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        assert!(!topic.is_null(), "topic create failed");
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        assert!(!publisher.is_null() && !subscriber.is_null());
        let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        let reader = zerodds_sub_create_datareader(subscriber, topic, ptr::null());
        assert!(!writer.is_null() && !reader.is_null());
        assert_eq!(
            zerodds_dw_wait_for_matched(writer, 1, 5_000),
            OK,
            "writer did not match the same-participant reader"
        );

        let payload = [1u8, 2, 3, 4];
        assert_eq!(
            zerodds_dw_write_w_timestamp(
                writer,
                payload.as_ptr(),
                payload.len(),
                0,
                1_234_567,
                250_000_000
            ),
            OK
        );
        assert_eq!(
            zerodds_dw_write_w_timestamp(
                writer,
                payload.as_ptr(),
                payload.len(),
                0,
                1,
                1_000_000_000
            ),
            BAD_PARAMETER,
            "a nanosecond field of a whole second is not a time"
        );

        let mut buf: *mut u8 = ptr::null_mut();
        let mut len: usize = 0;
        let mut info = ZeroDdsSampleInfo::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let rc = zerodds_dr_take_next_sample(reader, &mut buf, &mut len, &mut info);
            if rc == OK {
                break;
            }
            assert!(Instant::now() < deadline, "no sample arrived");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(std::slice::from_raw_parts(buf, len), payload.as_slice());
        assert_eq!(info.source_timestamp_sec, 1_234_567);
        // One RTPS fraction unit is 2^-32 s, below a nanosecond; the round
        // trip through it may land one nanosecond low.
        assert!(
            (249_999_999..=250_000_000).contains(&info.source_timestamp_nanosec),
            "nanosec {}",
            info.source_timestamp_nanosec
        );

        zerodds_sub_delete_datareader(subscriber, reader);
        zerodds_pub_delete_datawriter(publisher, writer);
        zerodds_dp_delete_subscriber(dp, subscriber);
        zerodds_dp_delete_publisher(dp, publisher);
        zerodds_dp_delete_topic(dp, topic);
        zerodds_dpf_delete_participant(f, dp);
    }
}
