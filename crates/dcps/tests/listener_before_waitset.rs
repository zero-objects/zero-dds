//! DDS 1.4 §2.2.4.6: "if both mechanisms are enabled, then the listener
//! mechanism is used first and then the WaitSet objects are signalled."
//!
//! On the path a delivered sample takes, not through a test hook. One test
//! in this binary: a WaitSet is woken by every data signal of the process,
//! and a signal of another test would wake it early.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_dcps::InstanceHandle;
use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::psm_constants::status as bits;
use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
use zerodds_dcps::{
    Condition, DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, RawBytes, SubscriberQos, TopicQos, WaitSet,
};

/// Takes its time, then notes that it ran.
struct SlowListener {
    done: Arc<AtomicBool>,
}

impl DataReaderListener for SlowListener {
    fn on_data_available(&self, _reader: InstanceHandle) {
        std::thread::sleep(Duration::from_millis(300));
        self.done.store(true, Ordering::SeqCst);
    }
}

/// A writer and a reader of one participant; the reader has a data
/// listener and a WaitSet waits on a ReadCondition of it. The WaitSet
/// wakes after the listener returned. It woke while the listener ran.
#[test]
fn the_listener_runs_before_the_waitset_is_signalled() {
    let p = DomainParticipantFactory::instance()
        .create_participant_with_config(
            common::unique_domain(),
            DomainParticipantQos::default(),
            common::isolated_cfg(),
        )
        .expect("participant");
    let topic = p
        .create_topic::<RawBytes>("ListenerFirst", TopicQos::default())
        .expect("topic");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("match");

    let done = Arc::new(AtomicBool::new(false));
    reader.set_listener(
        Some(Arc::new(SlowListener {
            done: Arc::clone(&done),
        })),
        bits::DATA_AVAILABLE,
    );
    let unread = reader
        .create_readcondition(
            sample_state_mask::NOT_READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .expect("read condition");
    let waitset = Arc::new(WaitSet::new());
    waitset
        .attach_condition(unread as Arc<dyn Condition>)
        .expect("attach");
    let waiting = Arc::clone(&waitset);
    let seen = Arc::clone(&done);
    let waiter = std::thread::spawn(move || {
        let woken = waiting.wait(Duration::from_secs(5));
        (woken.map(|c| c.len()), seen.load(Ordering::SeqCst))
    });
    std::thread::sleep(Duration::from_millis(100));

    writer.write(&RawBytes::new(vec![1, 2, 3])).expect("write");
    let (woken, listener_done) = waiter.join().expect("waiter");
    assert_eq!(woken.expect("the WaitSet woke"), 1);
    assert!(
        listener_done,
        "the WaitSet woke before on_data_available returned"
    );
}
