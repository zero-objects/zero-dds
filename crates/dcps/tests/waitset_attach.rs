//! DDS 1.4 §2.2.2.1.6.1 `attach_condition`: "if the Condition has a
//! trigger_value of TRUE, then attaching the condition will unblock the
//! WaitSet."

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::condition::{Condition, GuardCondition, WaitSet};
use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
use zerodds_dcps::{
    DataReaderQos, DomainParticipantFactory, DomainParticipantQos, RawBytes, SubscriberQos,
    TopicQos,
};

/// A WaitSet that holds only a read condition sleeps until a sample
/// arrives. A guard condition attached with trigger_value TRUE while it
/// waits wakes it, long before its timeout.
#[test]
fn attaching_a_triggered_condition_unblocks_a_waiting_waitset() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("Attached", TopicQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    let read = reader
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    let waitset = Arc::new(WaitSet::new());
    waitset
        .attach_condition(read as Arc<dyn Condition>)
        .unwrap();
    let waiting = Arc::clone(&waitset);
    let started = Instant::now();
    let thread = std::thread::spawn(move || waiting.wait(Duration::from_secs(5)));
    std::thread::sleep(Duration::from_millis(100));
    let guard = GuardCondition::new();
    guard.set_trigger_value(true);
    waitset
        .attach_condition(guard as Arc<dyn Condition>)
        .unwrap();
    let woken = thread.join().unwrap().expect("woken, not timed out");
    assert_eq!(woken.len(), 1);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "woke after {:?}",
        started.elapsed()
    );
}
