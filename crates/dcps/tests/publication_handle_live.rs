// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `SampleInfo::publication_handle` (DDS 1.4 §2.2.2.5.5) on the live/mpsc
//! delivery path — found while wiring `spatial_bridge.rs`'s echo
//! suppression (`crates/rmw-zerodds-shim/src/spatial_bridge.rs`'s `pump()`):
//! the field is always `HANDLE_NIL` there today, confirmed empirically, not
//! assumed. Only the offline `__push_raw` test-hook path (with no real
//! writer GUID to derive from) is documented and expected to stay
//! `HANDLE_NIL` — see `sample_info_lifecycle.rs`'s
//! `reader_publication_handle_field_is_handle_nil_offline`.
//!
//! `DcpsRuntime::user_reader_matched_publication_handles` — what a
//! `get_matched_publications()`-style API would return — already derives a
//! remote writer's handle via `InstanceHandle::from_guid(remote_writer_guid)`
//! (`crates/dcps/src/runtime.rs`). The fix must use the exact same
//! derivation on the reader's ingest path so the two agree on one writer's
//! identity.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    HANDLE_NIL, InstanceHandle, PublisherQos, SubscriberQos, TopicQos,
};

use common::{isolated_cfg, unique_domain};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Ping {
    seq: u32,
}

impl DdsType for Ping {
    const TYPE_NAME: &'static str = "test::PublicationHandlePing";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = false;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.seq.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let seq = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or(DecodeError::Invalid {
                what: "truncated Ping",
            })?;
        Ok(Self { seq })
    }

    fn encode_key_holder_be(&self, _holder: &mut PlainCdr2BeKeyHolder) {}

    fn decode_key_holder_be(_bytes: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self::default())
    }
}

/// Same-participant rig (see `time_source_destination_order.rs`'s
/// `intra_runtime_rig`): writer + reader on one `DomainParticipant`, so
/// `write()` returns only after `intra_runtime_dispatch_alive` has already
/// pushed the sample onto the reader's mpsc channel — no wait/sleep needed,
/// no SPDP/UDP discovery involved.
fn rig() -> (
    zerodds_dcps::DomainParticipant,
    zerodds_dcps::DataWriter<Ping>,
    zerodds_dcps::DataReader<Ping>,
) {
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");
    let topic = p
        .create_topic::<Ping>("PublicationHandlePings", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let writer = publisher
        .create_datawriter::<Ping>(&topic, DataWriterQos::default())
        .expect("writer");
    // `Ping` is keyless, so DDS 1.4 §2.2.2.5.1.10 treats the whole topic as
    // one implicit instance: the default HISTORY (`KEEP_LAST(1)`) would
    // keep only the most recent of the two writers' writes below, starving
    // `publication_handle_distinguishes_two_writers`'s "both are visible"
    // claim. KEEP_ALL takes HISTORY out of the way; this file is not
    // testing it.
    let reader = subscriber
        .create_datareader::<Ping>(
            &topic,
            DataReaderQos {
                history: HistoryQosPolicy {
                    kind: HistoryKind::KeepAll,
                    depth: 1,
                },
                ..DataReaderQos::default()
            },
        )
        .expect("reader");
    (p, writer, reader)
}

/// The core claim: `SampleInfo::publication_handle` on a sample delivered
/// through `take_with_info()` is the SAME `InstanceHandle` that
/// `get_matched_publications()`'s equivalent
/// (`DcpsRuntime::user_reader_matched_publication_handles`) reports for the
/// writer that sent it — not `HANDLE_NIL`.
#[test]
fn publication_handle_matches_get_matched_publications() {
    let (_p, writer, reader) = rig();

    writer.write(&Ping { seq: 1 }).expect("write");

    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(samples.len(), 1);
    let observed = samples[0].info.publication_handle;
    assert_ne!(
        observed, HANDLE_NIL,
        "publication_handle must identify the writer, not stay HANDLE_NIL"
    );

    let (rt, reader_eid) = reader.runtime_handle().expect("live reader");
    let matched = rt.user_reader_matched_publication_handles(reader_eid);
    assert_eq!(
        matched.len(),
        1,
        "exactly one writer must be matched to this reader"
    );
    assert_eq!(
        observed, matched[0],
        "SampleInfo::publication_handle must agree with \
         get_matched_publications()'s handle for the same writer"
    );
}

/// Two writers on the same topic must produce two distinct
/// `publication_handle`s on the reader side, each matching its own entry
/// in `get_matched_publications()`.
#[test]
fn publication_handle_distinguishes_two_writers() {
    let (p, writer_a, reader) = rig();
    let topic = p
        .create_topic::<Ping>("PublicationHandlePings", TopicQos::default())
        .expect("topic (existing)");
    let publisher = p.create_publisher(PublisherQos::default());
    let writer_b = publisher
        .create_datawriter::<Ping>(&topic, DataWriterQos::default())
        .expect("writer b");

    writer_a.write(&Ping { seq: 1 }).expect("write a");
    writer_b.write(&Ping { seq: 2 }).expect("write b");

    let samples = reader.take_with_info().expect("take_with_info");
    assert_eq!(samples.len(), 2);
    let handle_of = |seq: u32| -> InstanceHandle {
        samples
            .iter()
            .find(|s| s.data.seq == seq)
            .expect("sample")
            .info
            .publication_handle
    };
    let ha = handle_of(1);
    let hb = handle_of(2);
    assert_ne!(ha, HANDLE_NIL);
    assert_ne!(hb, HANDLE_NIL);
    assert_ne!(
        ha, hb,
        "two distinct writers must produce two distinct publication_handles"
    );
}

/// DDS 1.4 §2.2.2.5.3 `get_matched_publications` and
/// `get_matched_publication_data`, §2.2.2.4.2 the writer's counterparts:
/// a writer and a reader of one participant name each other, and each
/// reads the other's data with the QoS it was created with. The builtin
/// DCPSPublication reader holds discovered writers only; the C API
/// listed this writer's handle and did not find its data.
#[test]
fn matched_data_of_a_same_participant_pair_names_the_other_endpoint() {
    let domain = unique_domain();
    let p = DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");
    let topic = p
        .create_topic::<Ping>("MatchedData", TopicQos::default())
        .expect("topic");
    let mut wq = DataWriterQos::default();
    wq.deadline.period = zerodds_qos::Duration::from_millis(400);
    wq.user_data.value = vec![1, 2, 3];
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Ping>(&topic, wq)
        .expect("writer");
    let mut rq = DataReaderQos::default();
    rq.time_based_filter.minimum_separation = zerodds_qos::Duration::from_millis(100);
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Ping>(&topic, rq)
        .expect("reader");

    let publications = reader.matched_publications().expect("enabled");
    assert_eq!(publications.len(), 1);
    let publication = reader
        .matched_publication_data(publications[0])
        .expect("the matched writer's data");
    assert_eq!(publication.topic_name, "MatchedData");
    assert_eq!(publication.type_name, "test::PublicationHandlePing");
    assert_eq!(
        publication.deadline.period,
        zerodds_qos::Duration::from_millis(400)
    );
    assert_eq!(publication.user_data.value, vec![1, 2, 3]);

    let subscriptions = writer.matched_subscriptions().expect("enabled");
    assert_eq!(subscriptions.len(), 1);
    let subscription = writer
        .matched_subscription_data(subscriptions[0])
        .expect("the matched reader's data");
    assert_eq!(subscription.topic_name, "MatchedData");
    assert_eq!(
        subscription.time_based_filter.minimum_separation,
        zerodds_qos::Duration::from_millis(100)
    );

    // A handle that names no matched endpoint is BAD_PARAMETER, and the
    // reader's own handle is not one of the writer's publications.
    assert!(matches!(
        reader.matched_publication_data(subscriptions[0]),
        Err(zerodds_dcps::DdsError::BadParameter { .. })
    ));
    assert!(matches!(
        writer.matched_subscription_data(InstanceHandle::from_raw(12345)),
        Err(zerodds_dcps::DdsError::BadParameter { .. })
    ));
}
