// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-async-1.0 §3 — the waker model, on a live participant.
//!
//! §3.1 puts a waker slot on every `UserReaderSlot`, §3.2 has the receive
//! path fire it after `sample_tx.send`, §3.3 has `Stream::poll_next`
//! register `cx.waker()` in that slot on its `Pending` branch. All three
//! only differ from a polling loop if the waker a caller supplies is the
//! one that actually gets woken by sample arrival — that is what these
//! tests assert, with a counting waker of their own.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use futures_core::Stream;
use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy};
use zerodds_dcps::{DataReaderQos, DataWriterQos, RawBytes, TopicQos};
use zerodds_dcps_async::{AsyncDomainParticipantFactory, PublisherQos, SubscriberQos};

const MATCH_TIMEOUT: Duration = Duration::from_secs(5);
const WAKE_TIMEOUT: Duration = Duration::from_secs(5);

struct CountingWaker(Arc<AtomicUsize>);

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn counting_waker() -> (Waker, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    (
        Waker::from(Arc::new(CountingWaker(Arc::clone(&hits)))),
        hits,
    )
}

fn reliable_keepall_writer() -> DataWriterQos {
    DataWriterQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(500),
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..Default::default()
    }
}

fn reliable_keepall_reader() -> DataReaderQos {
    DataReaderQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(500),
        },
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..Default::default()
    }
}

/// Waits until `hits` has advanced, or gives up.
fn wait_for_wake(hits: &AtomicUsize, timeout: Duration) -> usize {
    let deadline = Instant::now() + timeout;
    loop {
        let n = hits.load(Ordering::SeqCst);
        if n > 0 || Instant::now() >= deadline {
            return n;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// §3.1 + §3.2 — a waker registered on the reader slot is woken by the
/// arrival of a sample, not by a timer.
#[tokio::test(flavor = "multi_thread")]
async fn reader_slot_waker_is_woken_on_sample_arrival() {
    let factory = AsyncDomainParticipantFactory::instance();
    let p = factory.create_participant(0).expect("participant");
    let topic = p
        .create_topic::<RawBytes>("AsyncWakerSlot", TopicQos::default())
        .expect("topic");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, reliable_keepall_writer())
        .expect("writer");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, reliable_keepall_reader())
        .expect("reader");

    writer
        .wait_for_matched_subscription(1, MATCH_TIMEOUT)
        .await
        .expect("writer match");
    reader
        .wait_for_matched_publication(1, MATCH_TIMEOUT)
        .await
        .expect("reader match");

    // §3.1: the reader is live, so it hands out the runtime + entity id
    // whose slot carries the waker.
    let (rt, eid) = reader
        .as_sync()
        .runtime_handle()
        .expect("a live reader has a runtime handle");

    let (waker, hits) = counting_waker();
    rt.register_user_reader_waker(eid, Some(waker));
    assert_eq!(hits.load(Ordering::SeqCst), 0, "no sample, no wake yet");

    writer
        .write(&RawBytes::new(vec![7, 7, 7]))
        .await
        .expect("write");

    // §3.2: the receive path wakes it.
    assert!(
        wait_for_wake(&hits, WAKE_TIMEOUT) > 0,
        "sample arrival must wake the registered waker"
    );

    // Clearing the slot is what an async reader does when it goes away;
    // afterwards no further wake reaches the (dead) consumer.
    rt.register_user_reader_waker(eid, None);
}

/// §3.3 — `SampleStream::poll_next` registers the caller's waker on its
/// `Pending` branch: the very waker handed in through `Context` is the one
/// that gets woken when a sample lands.
#[tokio::test(flavor = "multi_thread")]
async fn take_stream_poll_next_registers_the_callers_waker() {
    let factory = AsyncDomainParticipantFactory::instance();
    let p = factory.create_participant(0).expect("participant");
    let topic = p
        .create_topic::<RawBytes>("AsyncWakerStream", TopicQos::default())
        .expect("topic");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&topic, reliable_keepall_writer())
        .expect("writer");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, reliable_keepall_reader())
        .expect("reader");

    writer
        .wait_for_matched_subscription(1, MATCH_TIMEOUT)
        .await
        .expect("writer match");
    reader
        .wait_for_matched_publication(1, MATCH_TIMEOUT)
        .await
        .expect("reader match");

    let (waker, hits) = counting_waker();
    let mut cx = Context::from_waker(&waker);
    let mut stream = std::pin::pin!(reader.take_stream());

    // No sample yet → Pending, and the stream has put our waker in the
    // reader slot.
    assert!(
        matches!(stream.as_mut().poll_next(&mut cx), Poll::Pending),
        "an empty reader must yield Pending, not an item"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 0);

    writer
        .write(&RawBytes::new(vec![0xAB]))
        .await
        .expect("write");

    assert!(
        wait_for_wake(&hits, WAKE_TIMEOUT) > 0,
        "the waker passed to poll_next must be woken by sample arrival"
    );

    // And the next poll then produces the sample.
    let deadline = Instant::now() + WAKE_TIMEOUT;
    loop {
        match stream.as_mut().poll_next(&mut cx) {
            Poll::Ready(Some(sample)) => {
                assert_eq!(sample.data, vec![0xAB]);
                break;
            }
            Poll::Ready(None) => panic!("stream ended"),
            Poll::Pending => {
                assert!(Instant::now() < deadline, "sample never surfaced");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
