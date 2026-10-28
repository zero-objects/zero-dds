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
    /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
    const XCDR1_MATCHES_XCDR2: bool = true;
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

    /// The inverse of the holder above. DDS 1.4 §2.2.2.5.1.13 declares the
    /// key fields valid on a dispose/unregister sample; the holder is a
    /// PLAIN_CDR2-BE `uint32` and nothing else, so it is read as those four
    /// octets rather than through `Self::decode`, which reads the type's
    /// own stream.
    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let head: [u8; 4] =
            bytes
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .ok_or(DecodeError::Invalid {
                    what: "truncated KeyedRecord key holder",
                })?;
        Ok(Self {
            id: u32::from_be_bytes(head),
        })
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
async fn subscription_matched_stream_yields_initial_count() {
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

    let stream = reader.subscription_matched_stream();
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

// ---------------------------------------------------------------------
// A sample type with a readable field, for the content-filter items.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Reading {
    id: u32,
    value: i32,
}

impl DdsType for Reading {
    const TYPE_NAME: &'static str = "test::Reading";
    /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_be_bytes());
        out.extend_from_slice(&self.value.to_be_bytes());
        Ok(())
    }
    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 8 {
            return Err(DecodeError::Invalid { what: "truncated" });
        }
        Ok(Self {
            id: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            value: i32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        })
    }
    fn encode_key_holder_be(&self, h: &mut PlainCdr2BeKeyHolder) {
        h.write_u32(self.id);
    }
    fn field_value(&self, path: &str) -> Option<zerodds_dcps::FilterValue> {
        match path {
            "id" => Some(zerodds_dcps::FilterValue::Int(i64::from(self.id))),
            "value" => Some(zerodds_dcps::FilterValue::Int(i64::from(self.value))),
            _ => None,
        }
    }
}

fn encoded(r: &Reading) -> Vec<u8> {
    let mut out = Vec::new();
    r.encode(&mut out).expect("encode");
    out
}

// ---------------------------------------------------------------------
// §1 Type mapping to the sync API.
// ---------------------------------------------------------------------

/// Spec §1 — `AsyncDomainParticipant` is a newtype over the sync
/// participant: topic, publisher and subscriber creation are the sync
/// operations, and `as_sync()` hands the same participant back.
#[tokio::test(flavor = "multi_thread")]
async fn participant_creates_topic_publisher_and_subscriber() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(7);
    assert_eq!(p.domain_id(), 7);
    assert_eq!(p.as_sync().domain_id(), 7);

    let topic = p
        .create_topic::<Reading>("AT_MAP", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let sub = p.create_subscriber(SubscriberQos::default());
    let writer = pubr
        .create_datawriter::<Reading>(&topic, DataWriterQos::default())
        .expect("writer");
    let reader = sub
        .create_datareader::<Reading>(&topic, DataReaderQos::default())
        .expect("reader");

    // Spec §1 table: `Topic<T>` is identical in both worlds — it is the
    // sync topic that both endpoints were built from.
    assert_eq!(writer.as_sync().topic().name(), topic.name());
    assert_eq!(reader.as_sync().topic().name(), topic.name());
    assert_eq!(<Reading as DdsType>::TYPE_NAME, "test::Reading");
}

/// Spec §1 — every async newtype is `Clone + Send + Sync` and a clone
/// shares the inner state rather than duplicating it: a write through one
/// handle is visible through the other.
#[tokio::test(flavor = "multi_thread")]
async fn async_newtypes_are_clone_send_sync_and_share_state() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<zerodds_dcps_async::AsyncDomainParticipant>();
    assert_send_sync::<zerodds_dcps_async::AsyncPublisher>();
    assert_send_sync::<zerodds_dcps_async::AsyncSubscriber>();
    assert_send_sync::<zerodds_dcps_async::AsyncDataWriter<Reading>>();
    assert_send_sync::<zerodds_dcps_async::AsyncDataReader<Reading>>();

    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_CLONE", TopicQos::default())
        .expect("topic");
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Reading>(&topic, DataReaderQos::default())
        .expect("reader");
    let clone = reader.clone();

    reader
        .as_sync()
        .__push_raw(encoded(&Reading { id: 1, value: 5 }))
        .expect("push");

    // The clone drains the sample the original was pushed into: both
    // handles point at one reader.
    let got = clone.take(Duration::from_millis(200)).await.expect("take");
    assert_eq!(got, vec![Reading { id: 1, value: 5 }]);
    let after = reader.take(Duration::from_millis(20)).await.expect("take");
    assert!(
        after.is_empty(),
        "the sample was already taken by the clone"
    );
}

// ---------------------------------------------------------------------
// §2 Method signatures.
// ---------------------------------------------------------------------

/// Spec §2.1.6 — `matched_subscription_count` is a synchronous snapshot,
/// not a future, and reports the same number as the sync writer.
#[tokio::test(flavor = "multi_thread")]
async fn matched_subscription_count_is_a_synchronous_snapshot() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_MSC", TopicQos::default())
        .expect("topic");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Reading>(&topic, DataWriterQos::default())
        .expect("writer");

    // No `.await` — the call is a plain accessor.
    let n: usize = writer.matched_subscription_count();
    assert_eq!(n, 0);
    assert_eq!(n, writer.as_sync().matched_subscription_count());
}

/// Spec §2.2.4 — `matched_publication_count` likewise.
#[tokio::test(flavor = "multi_thread")]
async fn matched_publication_count_is_a_synchronous_snapshot() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_MPC", TopicQos::default())
        .expect("topic");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(&topic, DataReaderQos::default())
        .expect("reader");

    let n: usize = reader.matched_publication_count();
    assert_eq!(n, 0);
    assert_eq!(n, reader.as_sync().matched_publication_count());
}

/// Spec §2.2.3 — `wait_for_matched_publication` resolves `Err(Timeout)`
/// when `min_count` is never reached, and `Ok` when it already is.
#[tokio::test(flavor = "multi_thread")]
async fn wait_for_matched_publication_times_out_without_writer() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_WFMP", TopicQos::default())
        .expect("topic");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(&topic, DataReaderQos::default())
        .expect("reader");

    let start = std::time::Instant::now();
    let res = reader
        .wait_for_matched_publication(1, Duration::from_millis(100))
        .await;
    assert!(matches!(res, Err(zerodds_dcps_async::DdsError::Timeout)));
    assert!(start.elapsed() >= Duration::from_millis(80));

    // `min_count = 0` is already satisfied → resolves immediately.
    reader
        .wait_for_matched_publication(0, Duration::from_millis(100))
        .await
        .expect("min_count 0 is met");
}

/// Spec §2.2.1 — `take_stream` yields the samples in arrival order.
#[tokio::test(flavor = "multi_thread")]
async fn take_stream_yields_pushed_samples() {
    use futures::StreamExt;

    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_STREAM", TopicQos::default())
        .expect("topic");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(&topic, keep_all_reader_qos())
        .expect("reader");

    for i in 0..3i32 {
        reader
            .as_sync()
            .__push_raw(encoded(&Reading {
                id: 1,
                value: 10 + i,
            }))
            .expect("push");
    }

    let mut stream = reader.take_stream();
    let mut got = Vec::new();
    for _ in 0..3 {
        let item = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("stream timeout")
            .expect("stream ended");
        got.push(item.value);
    }
    assert_eq!(got, vec![10, 11, 12]);
}

/// Spec §2.2.1 — the `Stream<Item = Sample<T>>` form the spec text names:
/// every item carries its `SampleInfo`.
#[tokio::test(flavor = "multi_thread")]
async fn take_stream_with_info_yields_sample_info() {
    use futures::StreamExt;

    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_STREAM_INFO", TopicQos::default())
        .expect("topic");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(&topic, DataReaderQos::default())
        .expect("reader");

    reader
        .as_sync()
        .__push_raw(encoded(&Reading { id: 4, value: 42 }))
        .expect("push");

    let mut stream = reader.take_stream_with_info();
    let sample = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("stream timeout")
        .expect("stream ended");
    assert_eq!(sample.data.value, 42);
    assert!(sample.info.valid_data, "an alive sample carries valid_data");
}

/// Spec §2.2.2 — `take_with_info` returns `Vec<Sample<T>>`, the spec's
/// signature, with the metadata attached.
#[tokio::test(flavor = "multi_thread")]
async fn take_with_info_carries_sample_info() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_TWI", TopicQos::default())
        .expect("topic");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(&topic, DataReaderQos::default())
        .expect("reader");

    reader
        .as_sync()
        .__push_raw(encoded(&Reading { id: 9, value: -3 }))
        .expect("push");

    let samples = reader
        .take_with_info(Duration::from_millis(500))
        .await
        .expect("take_with_info");
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].data, Reading { id: 9, value: -3 });
    assert!(samples[0].info.valid_data);

    // `take` consumed it — a second call comes back empty.
    let again = reader
        .take_with_info(Duration::from_millis(20))
        .await
        .expect("take_with_info");
    assert!(again.is_empty());
}

/// Spec §2.2.2 — `read` is the non-consuming counterpart: the sample stays
/// in the reader and a later `take` still finds it.
#[tokio::test(flavor = "multi_thread")]
async fn read_is_non_consuming() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_READ", TopicQos::default())
        .expect("topic");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(&topic, DataReaderQos::default())
        .expect("reader");

    reader
        .as_sync()
        .__push_raw(encoded(&Reading { id: 2, value: 77 }))
        .expect("push");

    let first = reader.read(Duration::from_millis(500)).await.expect("read");
    assert_eq!(first, vec![Reading { id: 2, value: 77 }]);
    let second = reader.read(Duration::from_millis(500)).await.expect("read");
    assert_eq!(second, first, "read must not consume the sample");
    let taken = reader.take(Duration::from_millis(500)).await.expect("take");
    assert_eq!(taken, first, "take still finds what read left behind");
}

/// A reader that keeps every sample. The tests below push several samples
/// of one instance (`id: 1`), and DDS 1.4 §2.2.3's default HISTORY,
/// KEEP_LAST(1), keeps only the newest of them since the reader enforces
/// HISTORY (44723842029): the filter test saw `[30]` of `[20, 30]` and the
/// stream test waited for samples that had been replaced. What they measure
/// is the filter and the stream, not the history.
fn keep_all_reader_qos() -> DataReaderQos {
    DataReaderQos {
        history: zerodds_qos::HistoryQosPolicy {
            kind: zerodds_qos::HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

/// Spec §2.2 (content filter extension) — a closure-filtered async reader
/// surfaces only the matching samples.
#[tokio::test(flavor = "multi_thread")]
async fn filtered_reader_delivers_only_matching() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_FILTER", TopicQos::default())
        .expect("topic");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader_filtered::<Reading, _>(&topic, keep_all_reader_qos(), |r: &Reading| {
            r.value > 10
        })
        .expect("filtered reader");

    for v in [5i32, 20, 7, 30] {
        reader
            .as_sync()
            .__push_raw(encoded(&Reading { id: 1, value: v }))
            .expect("push");
    }

    let got = reader.take(Duration::from_millis(500)).await.expect("take");
    let values: Vec<i32> = got.iter().map(|r| r.value).collect();
    assert_eq!(values, vec![20, 30]);
}

/// Spec §2.2 (content filter extension) — the same through a
/// `ContentFilteredTopic` with an SQL expression and a `%0` parameter.
#[tokio::test(flavor = "multi_thread")]
async fn sql_content_filtered_reader_delivers_only_matching() {
    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_CFT", TopicQos::default())
        .expect("topic");
    let cft = p
        .create_contentfilteredtopic::<Reading>(
            "AT_CFT_HIGH",
            &topic,
            "value > %0",
            vec!["10".to_string()],
        )
        .expect("cft");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader_cft::<Reading>(&cft, keep_all_reader_qos())
        .expect("cft reader");

    for v in [5i32, 20, 7, 30] {
        reader
            .as_sync()
            .__push_raw(encoded(&Reading { id: 1, value: v }))
            .expect("push");
    }

    let got = reader.take(Duration::from_millis(500)).await.expect("take");
    let values: Vec<i32> = got.iter().map(|r| r.value).collect();
    assert_eq!(values, vec![20, 30]);
}

// ---------------------------------------------------------------------
// §6 Listener bridge.
// ---------------------------------------------------------------------

/// Spec §6.2 — the writer-side stream yields a full
/// `PublicationMatchedStatus`, the type the spec names.
#[tokio::test(flavor = "multi_thread")]
async fn writer_publication_matched_stream_yields_status() {
    use core::future::poll_fn;
    use futures_core::Stream;

    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_PMS", TopicQos::default())
        .expect("topic");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Reading>(&topic, DataWriterQos::default())
        .expect("writer");

    let stream = writer.publication_matched_stream();
    let mut s = std::pin::pin!(stream);
    let first = poll_fn(|cx| s.as_mut().poll_next(cx));
    let status: zerodds_dcps_async::PublicationMatchedStatus =
        tokio::time::timeout(Duration::from_millis(500), first)
            .await
            .expect("stream timeout")
            .expect("stream ended");
    assert_eq!(status.current_count, 0);
    assert_eq!(status.total_count, 0);
    assert_eq!(status.current_count_change, 0);
}

// ---------------------------------------------------------------------
// Goals — runtime agnosticism.
// ---------------------------------------------------------------------

/// Spec "Ziele" / §3 — the public API is runtime-agnostic: the futures it
/// returns are ordinary `Future`s that a hand-written executor with no
/// tokio anywhere in it can drive to completion.
///
/// The `tokio-glue` feature is the deliberate exception: it routes
/// `yield_for` through `tokio::time::sleep`, which needs a tokio reactor.
/// The claim is about the default build, so that is where it is asserted.
#[cfg(not(feature = "tokio-glue"))]
#[test]
fn futures_complete_on_a_non_tokio_executor() {
    use std::future::Future;
    use std::sync::{Arc, Condvar, Mutex};
    use std::task::{Context, Poll, Wake, Waker};

    /// The minimal executor: park the calling thread, let the waker
    /// unpark it. No runtime, no reactor, no tokio.
    struct Signal {
        woken: Mutex<bool>,
        cv: Condvar,
    }
    impl Wake for Signal {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            *self.woken.lock().expect("lock") = true;
            self.cv.notify_all();
        }
    }
    fn block_on<F: Future>(fut: F) -> F::Output {
        let signal = Arc::new(Signal {
            woken: Mutex::new(false),
            cv: Condvar::new(),
        });
        let waker = Waker::from(Arc::clone(&signal));
        let mut cx = Context::from_waker(&waker);
        let mut fut = std::pin::pin!(fut);
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
            let mut woken = signal.woken.lock().expect("lock");
            while !*woken {
                woken = signal.cv.wait(woken).expect("wait");
            }
            *woken = false;
        }
    }

    let f = AsyncDomainParticipantFactory::instance();
    let p = f.create_participant_offline(0);
    let topic = p
        .create_topic::<Reading>("AT_NOTOKIO", TopicQos::default())
        .expect("topic");
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Reading>(&topic, DataWriterQos::default())
        .expect("writer");
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Reading>(&topic, DataReaderQos::default())
        .expect("reader");

    block_on(writer.write(&Reading { id: 1, value: 3 })).expect("write");
    reader
        .as_sync()
        .__push_raw(encoded(&Reading { id: 1, value: 3 }))
        .expect("push");
    let got = block_on(reader.take(Duration::from_millis(500))).expect("take");
    assert_eq!(got, vec![Reading { id: 1, value: 3 }]);

    // The timeout path also has to make progress without tokio: this
    // future only completes because `yield_for`'s waker fires.
    let res = block_on(writer.wait_for_matched_subscription(1, Duration::from_millis(50)));
    assert!(matches!(res, Err(zerodds_dcps_async::DdsError::Timeout)));
}
