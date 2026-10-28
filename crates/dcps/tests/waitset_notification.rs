//! The wait-based half of the subscription view (DDS 1.4 §2.2.6.2.2), and
//! the case where both notification mechanisms are on at once (§2.2.4.6).
//!
//! Figure 2.23 of the specification is the sequence these tests walk: the
//! application creates a `WaitSet` and a `ReadCondition`, attaches the one
//! to the other and blocks in `wait`; a sample reaches the reader; the
//! condition turns true, the wait unblocks and returns that condition, and
//! the application reads the sample in its own thread.
//!
//! §2.2.4.6 adds the combination rule: when a listener and a wait-set are
//! both enabled, the listener runs first and the wait-sets are signalled
//! afterwards. Both must see the same arrival.
//!
//! Offline participants throughout — the notification path is independent
//! of the transport.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use zerodds_dcps::InstanceHandle;
use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::psm_constants::status as bits;
use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
use zerodds_dcps::{
    Condition, DataReader, DataReaderQos, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, RawBytes, ReadCondition, Subscriber, SubscriberQos, TopicQos, WaitSet,
};

#[derive(Default)]
struct AvailableCounter {
    hits: AtomicU32,
}

impl DataReaderListener for AvailableCounter {
    fn on_data_available(&self, _r: InstanceHandle) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }
}

/// An offline participant with one subscriber and one `RawBytes` reader.
fn reader_setup(topic: &str) -> (DomainParticipant, Subscriber, DataReader<RawBytes>) {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p
        .create_topic::<RawBytes>(topic, TopicQos::default())
        .expect("create_topic");
    let subr = p.create_subscriber(SubscriberQos::default());
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .expect("create_datareader");
    (p, subr, r)
}

/// A `ReadCondition` that is true exactly while the reader holds an
/// undelivered sample — the specification's "presence of at least one
/// sample managed by the Service" (§2.2.4.5), expressed over the reader's
/// inbox.
fn undelivered_sample_condition(reader: &DataReader<RawBytes>) -> Arc<ReadCondition> {
    let inbox = reader.__inbox_handle();
    ReadCondition::new(
        sample_state_mask::NOT_READ,
        view_state_mask::ANY,
        instance_state_mask::ALIVE,
        move |_, _, _| {
            inbox
                .lock()
                .map(|held| !held.is_empty())
                .unwrap_or_default()
        },
    )
}

/// §2.2.6.2.2: a `WaitSet` blocked on a `ReadCondition` of a reader is
/// unblocked by the arrival of a sample on that reader, returns that
/// condition as the triggered one, and the sample is then read in the
/// application's own thread.
#[test]
fn data_arrival_unblocks_a_waitset_waiting_on_a_read_condition() {
    let (_p, _subr, r) = reader_setup("WaitSetChatter");
    let cond = undelivered_sample_condition(&r);
    let ws = WaitSet::new();
    ws.attach_condition(Arc::clone(&cond) as Arc<dyn Condition>)
        .expect("attach");

    // Nothing has arrived: the wait blocks and times out rather than
    // reporting a condition that is not true.
    assert!(
        ws.wait(Duration::from_millis(50)).is_err(),
        "an empty reader must not trigger the wait"
    );

    // The application thread blocks in `wait`; the arrival happens here.
    // `WaitSet` clones share one state, so the waiter and this thread are
    // the same wait-set.
    let waiter_set = ws.clone();
    let waiter = std::thread::spawn(move || waiter_set.wait(Duration::from_secs(5)));
    std::thread::sleep(Duration::from_millis(50));
    r.__push_raw(vec![7, 8, 9]).expect("push");

    let active = waiter
        .join()
        .expect("waiter thread")
        .expect("the arrival must unblock the wait");
    assert_eq!(active.len(), 1, "exactly the read condition triggered");
    assert!(
        Arc::ptr_eq(&active[0], &(Arc::clone(&cond) as Arc<dyn Condition>)),
        "wait must return the attached condition itself"
    );

    // Third part of Figure 2.23: the read runs in the user's context after
    // the wait returned.
    let samples = r.take().expect("take");
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].data, vec![7, 8, 9]);
    // And the condition is false again once the sample has been consumed.
    assert!(!cond.get_trigger_value());
}

/// §2.2.4.6: with both mechanisms enabled the listener runs first and the
/// wait-sets are signalled afterwards — one arrival, both notified.
#[test]
fn a_listener_and_a_waitset_both_see_the_same_arrival() {
    let (_p, _subr, r) = reader_setup("CombinedChatter");
    let counter = Arc::new(AvailableCounter::default());
    r.set_listener(
        Some(Arc::clone(&counter) as Arc<dyn DataReaderListener>),
        bits::ANY,
    );

    let cond = undelivered_sample_condition(&r);
    let ws = WaitSet::new();
    ws.attach_condition(Arc::clone(&cond) as Arc<dyn Condition>)
        .expect("attach");

    // `__push_raw` runs the notification path a delivered sample runs:
    // the listener chain first, then the wait-set signal.
    r.__push_raw(vec![1, 2, 3]).expect("push");

    assert_eq!(
        counter.hits.load(Ordering::Relaxed),
        1,
        "the listener mechanism is used first"
    );
    let active = ws
        .wait(Duration::from_secs(5))
        .expect("and the wait-set is signalled after it");
    assert_eq!(active.len(), 1);
}
