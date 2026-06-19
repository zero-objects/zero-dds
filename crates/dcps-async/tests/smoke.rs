//! Smoke tests for the async API. Offline path — no UDP, just
//! sync reuse via a newtype.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::time::Duration;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::{DdsType, RawBytes};
use zerodds_dcps_async::{
    AsyncDomainParticipantFactory, DataReaderQos, DataWriterQos, PublisherQos, SubscriberQos,
    TopicQos,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Keyed {
    id: u32,
}

impl DdsType for Keyed {
    const TYPE_NAME: &'static str = "test::Keyed";
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_be_bytes());
        Ok(())
    }
    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 4 {
            return Err(DecodeError::Invalid { what: "truncated" });
        }
        Ok(Self {
            id: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        })
    }
    fn encode_key_holder_be(&self, h: &mut PlainCdr2BeKeyHolder) {
        h.write_u32(self.id);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn factory_singleton_offline_participant() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    assert_eq!(p.domain_id(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn writer_write_async_offline() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<RawBytes>("AT", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = pubr
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");

    writer
        .write(&RawBytes::new(vec![1, 2, 3]))
        .await
        .expect("async write");
}

#[tokio::test(flavor = "multi_thread")]
async fn reader_take_returns_empty_after_timeout() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<RawBytes>("AT2", TopicQos::default())
        .expect("topic");
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader");

    let samples = reader.take(Duration::from_millis(50)).await.expect("take");
    assert!(samples.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn writer_register_dispose_unregister_async() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Keyed>("AT3", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = pubr
        .create_datawriter::<Keyed>(&topic, DataWriterQos::default())
        .expect("writer");

    let s = Keyed { id: 7 };
    let h = writer.register_instance(&s).await.expect("register");
    writer.write(&s).await.expect("write");
    writer.dispose(&s, h).await.expect("dispose");
    writer.unregister_instance(&s, h).await.expect("unregister");
}

#[tokio::test(flavor = "multi_thread")]
async fn publication_matched_stream_yields_initial_count() {
    use core::future::poll_fn;
    use futures_core::Stream;
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<RawBytes>("AT5", TopicQos::default())
        .expect("topic");
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader");

    let stream = reader.publication_matched_stream();
    // First poll — should return the initial `SubscriptionMatchedStatus`
    // (current_count=0, total_count=0), since `last_count=MAX` is
    // the initial value and the delta jump is detected.
    let mut s = std::pin::pin!(stream);
    let first = poll_fn(|cx| s.as_mut().poll_next(cx));
    let v = tokio::time::timeout(Duration::from_millis(200), first)
        .await
        .expect("timeout")
        .expect("none");
    assert_eq!(v.current_count, 0);
    assert_eq!(v.total_count, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn write_returns_timeout_after_max_blocking_when_queue_full() {
    // Spec §5.1: On OutOfResources with max_blocking_time = 50ms,
    // AsyncDataWriter::write must return a Timeout after ~50ms (instead
    // of blocking synchronously).
    use zerodds_qos::Duration as QosDuration;
    use zerodds_qos::ReliabilityKind;
    use zerodds_qos::ReliabilityQosPolicy;

    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<RawBytes>("AT_BP", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());

    let qos = DataWriterQos {
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: QosDuration::from_millis(50),
        },
        resource_limits: zerodds_qos::ResourceLimitsQosPolicy {
            max_samples: 1,
            max_instances: 1,
            max_samples_per_instance: 1,
        },
        history: zerodds_qos::HistoryQosPolicy {
            kind: zerodds_qos::HistoryKind::KeepAll,
            depth: 1,
        },
        ..Default::default()
    };
    let writer = pubr
        .create_datawriter::<RawBytes>(&topic, qos)
        .expect("writer");

    // One sample fills the queue.
    writer
        .write(&RawBytes::new(vec![1]))
        .await
        .expect("first write");
    // Second sample → OutOfResources → retry loop → Timeout after ~50ms.
    let start = std::time::Instant::now();
    let res = writer.write(&RawBytes::new(vec![2])).await;
    let elapsed = start.elapsed();
    assert!(matches!(res, Err(zerodds_dcps_async::DdsError::Timeout)));
    assert!(
        elapsed >= Duration::from_millis(40),
        "timeout should wait ~50ms, was {elapsed:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn data_available_stream_signals_without_consuming() {
    // Spec zerodds-async-1.0 §6.1: data_available_stream emits
    // a `()` event per new sample and does NOT consume the
    // samples — the caller must call `take()`/`take_stream()`
    // separately to read the samples.
    use core::future::poll_fn;
    use futures_core::Stream;

    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<RawBytes>("AT_DA", TopicQos::default())
        .expect("topic");
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader");

    // Push one sample directly into the reader inbox (offline path).
    reader.as_sync().__push_raw(vec![0xAB, 0xCD]).expect("push");

    // data_available_stream must return `()`.
    let stream = reader.data_available_stream();
    let mut s = std::pin::pin!(stream);
    let event = poll_fn(|cx| s.as_mut().poll_next(cx));
    let _: () = tokio::time::timeout(Duration::from_millis(200), event)
        .await
        .expect("timeout")
        .expect("none");

    // Important: the sample is NOT consumed — `take()` returns it.
    let samples = reader.take(Duration::from_millis(50)).await.expect("take");
    assert_eq!(
        samples.len(),
        1,
        "sample must still be in the reader after data_available"
    );
    assert_eq!(samples[0].data, vec![0xAB, 0xCD]);
}

#[tokio::test(flavor = "multi_thread")]
async fn wait_for_matched_subscription_times_out_when_no_reader() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<RawBytes>("AT4", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = pubr
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");

    let res = writer
        .wait_for_matched_subscription(1, Duration::from_millis(100))
        .await;
    assert!(matches!(res, Err(zerodds_dcps_async::DdsError::Timeout)));
}
