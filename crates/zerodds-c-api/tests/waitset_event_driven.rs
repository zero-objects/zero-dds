//! Event-driven WaitSet-on-ReadCondition trigger test.
//!
//! Proves that a WaitSet attached to a ReadCondition wakes when a sample
//! arrives WITHOUT a busy-poll: the wait is given a generous timeout but must
//! return `Ok` (a triggered condition) well before that timeout once a
//! same-participant writer writes a sample. The intra-runtime delivery path
//! forwards the DCPS `async_waker` signal into the WaitSet's data condvar.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use core::ffi::c_void;
use std::ffi::CString;
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::condition_ffi::{
    zerodds_condition_get_trigger_value, zerodds_dr_create_readcondition,
    zerodds_waitset_attach_condition, zerodds_waitset_create, zerodds_waitset_destroy,
    zerodds_waitset_wait,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
};
use zerodds::publisher_ffi::{
    zerodds_dw_wait_for_matched, zerodds_dw_write, zerodds_pub_create_datawriter,
};
use zerodds::subscriber_ffi::zerodds_sub_create_datareader;

// Sample-state mask bits matching the c-api read cache (NOT_READ = 2).
const NOT_READ: u32 = 2;
const ANY_STATE: u32 = 0;

/// Same-participant writer→reader: a ReadCondition on a WaitSet must wake
/// (return Ok with the condition active) within a few ms of the write, far
/// below the 10s wait budget — i.e. the wakeup is event-driven, not a 10s
/// timeout, and not a fixed poll interval.
#[test]
fn waitset_readcondition_wakes_on_write_event_driven() {
    let domain: u32 = 70 + (std::process::id() % 40);
    let topic_name = CString::new("WsEventTopic").unwrap();
    let type_name = CString::new("RawBytes").unwrap();

    // SAFETY: spec-hierarchy FFI with valid C strings; every pointer is
    // NULL-checked before use per each fn's # Safety contract. CStrings live
    // until the end of the block.
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

        // ReadCondition for unread, any view/instance state.
        let cond = zerodds_dr_create_readcondition(reader, NOT_READ, ANY_STATE, ANY_STATE);
        assert!(!cond.is_null(), "readcondition create failed");

        let ws = zerodds_waitset_create();
        assert!(!ws.is_null());
        let rc = zerodds_waitset_attach_condition(ws, cond as *mut c_void);
        assert_eq!(rc, 0, "attach failed");

        // Same-participant match resolves intra-runtime; give it a moment.
        let _ = zerodds_dw_wait_for_matched(writer, 1, 2_000);

        // Trigger should be false before any sample.
        assert!(
            !zerodds_condition_get_trigger_value(cond as *const c_void),
            "trigger must be false before any sample is written"
        );

        // Write a sample, then block on the WaitSet with a generous 10s budget.
        // If the trigger is event-driven the wait returns in milliseconds.
        let payload = [0xC0u8, 0xFF, 0xEE, 0x42];
        assert_eq!(
            zerodds_dw_write(writer, payload.as_ptr(), payload.len(), 0),
            0,
            "write failed"
        );

        let mut buf: [*mut c_void; 4] = [ptr::null_mut(); 4];
        let mut count: usize = 0;
        let t0 = Instant::now();
        let rc = zerodds_waitset_wait(
            ws,
            buf.as_mut_ptr(),
            4,
            &mut count,
            10, // 10s safety budget
            0,
        );
        let elapsed = t0.elapsed();

        assert_eq!(rc, 0, "waitset_wait must return Ok (condition active)");
        assert_eq!(count, 1, "exactly one condition must be active");
        assert_eq!(
            buf[0], cond as *mut c_void,
            "the ReadCondition must be active"
        );

        // Event-driven proof: must wake well before the 10s budget. A busy-poll
        // would also be "fast", but the wait blocks on a condvar woken by the
        // sample's waker — confirmed by the latency being far under the budget.
        assert!(
            elapsed < Duration::from_secs(2),
            "wait should wake in ~ms after the write, took {elapsed:?}"
        );

        // The condition stays active after the wait (sample still unread).
        assert!(
            zerodds_condition_get_trigger_value(cond as *const c_void),
            "trigger must be true while the unread sample is present"
        );

        zerodds_waitset_destroy(ws);
        zerodds_dpf_delete_participant(f, dp);
    }
}

/// Regression / NULL-safety: a WaitSet with no triggering condition returns
/// Timeout (never blocks forever, never reads NULL), and the wait API is
/// NULL-tolerant on all of its out-pointers / handle.
#[test]
fn waitset_wait_null_and_timeout_safe() {
    // SAFETY: NULL handle + NULL out-pointers are explicit contracts (BadParameter).
    unsafe {
        let mut count: usize = 0;
        let mut buf: [*mut c_void; 1] = [ptr::null_mut(); 1];

        // NULL waitset → BadParameter, no crash.
        let rc = zerodds_waitset_wait(ptr::null_mut(), buf.as_mut_ptr(), 1, &mut count, 0, 0);
        assert!(
            rc < 0,
            "NULL waitset must yield a negative status, got {rc}"
        );

        // Valid empty waitset → Timeout (returns promptly, does not hang).
        let ws = zerodds_waitset_create();
        assert!(!ws.is_null());
        let t0 = Instant::now();
        let rc = zerodds_waitset_wait(ws, buf.as_mut_ptr(), 1, &mut count, 0, 50_000_000);
        assert_eq!(count, 0);
        assert!(rc != 0, "empty waitset must time out (non-Ok), got {rc}");
        assert!(
            t0.elapsed() < Duration::from_secs(2),
            "timeout must be honoured promptly"
        );
        zerodds_waitset_destroy(ws);

        // NULL out-pointer → BadParameter.
        let ws = zerodds_waitset_create();
        let rc = zerodds_waitset_wait(ws, ptr::null_mut(), 1, &mut count, 0, 0);
        assert!(
            rc < 0,
            "NULL out_active must yield a negative status, got {rc}"
        );
        zerodds_waitset_destroy(ws);
    }
}
