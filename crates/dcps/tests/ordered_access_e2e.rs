//! DDS 1.4 §2.2.3.6 PRESENTATION — `ordered_access` above INSTANCE scope.
//!
//! The clause under test: with `access_scope = TOPIC` (or GROUP) and
//! `ordered_access = true`, the Subscriber presents the samples in the order
//! they were written, **across** the instances of the topic. With the default
//! (`INSTANCE`, `ordered_access = false`) the ordering guarantee is per
//! instance only. The instance-cycling API returns one instance either way
//! (§2.2.2.5.3.16); the order across instances shows on the plain take.
//!
//! The same-runtime writer→reader route is a direct channel handoff, so this
//! needs no UDP loopback and no platform gate.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs
)]

#[path = "common/mod.rs"]
mod common;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::qos::{
    HistoryKind, HistoryQosPolicy, PresentationAccessScope, PresentationQosPolicy,
};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    HANDLE_NIL, PublisherQos, SubscriberQos, TopicQos,
};

use common::{isolated_cfg, unique_domain};

/// Two members: `id` is the instance key, `seq` the write order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Tick {
    id: u32,
    seq: u32,
}

impl DdsType for Tick {
    const TYPE_NAME: &'static str = "test::Tick";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 8 {
            return Err(DecodeError::Invalid {
                what: "truncated Tick",
            });
        }
        Ok(Self {
            id: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            seq: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
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
            ..Self::default()
        })
    }
}

fn presentation(scope: PresentationAccessScope, ordered: bool) -> PresentationQosPolicy {
    PresentationQosPolicy {
        access_scope: scope,
        coherent_access: false,
        ordered_access: ordered,
    }
}

/// Both rigs below write 2 samples to each of 2 instances before ever
/// taking — `DataWriterQos`/`DataReaderQos::default()`'s HISTORY is
/// `KEEP_LAST(1)` (DDS 1.4 §2.2.3.18 default), which would keep only the
/// second write of each instance and starve the very ordering/grouping
/// behaviour these tests exist to check. `depth: 2` is unrelated to what
/// is under test here, so it is exactly as high as this file's own fixed
/// write pattern needs, not "high enough to not matter".
fn keep_last_two_per_instance() -> HistoryQosPolicy {
    HistoryQosPolicy {
        kind: HistoryKind::KeepLast,
        depth: 2,
    }
}

/// Writes `(id=1,seq=1), (id=2,seq=2), (id=1,seq=3), (id=2,seq=4)` — two
/// instances, interleaved — and returns what the reader's instance-cycling API
/// hands back on its first call.
fn interleaved_first_batch(scope: PresentationAccessScope, ordered: bool) -> Vec<Tick> {
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");

    let publisher = participant.create_publisher(PublisherQos {
        presentation: presentation(scope, ordered),
        ..PublisherQos::default()
    });
    let subscriber = participant.create_subscriber(SubscriberQos {
        presentation: presentation(scope, ordered),
        ..SubscriberQos::default()
    });
    let topic = participant
        .create_topic::<Tick>("OrderedTicks", TopicQos::default())
        .expect("topic");
    let writer = publisher
        .create_datawriter::<Tick>(
            &topic,
            DataWriterQos {
                history: keep_last_two_per_instance(),
                ..DataWriterQos::default()
            },
        )
        .expect("writer");
    let reader = subscriber
        .create_datareader::<Tick>(
            &topic,
            DataReaderQos {
                history: keep_last_two_per_instance(),
                ..DataReaderQos::default()
            },
        )
        .expect("reader");

    for (id, seq) in [(1u32, 1u32), (2, 2), (1, 3), (2, 4)] {
        writer.write(&Tick { id, seq }).expect("write");
    }
    // Under GROUP access scope a take is inside begin_access/end_access
    // (DDS 1.4 §2.2.2.5.2.8); elsewhere the bracket has no effect.
    subscriber.begin_access().expect("begin_access");
    let batch = reader
        .take_next_instance(HANDLE_NIL)
        .expect("take_next_instance")
        .into_iter()
        .map(|s| s.data)
        .collect();
    subscriber.end_access().expect("end_access");
    batch
}

/// §2.2.2.5.3.16: `take_next_instance` returns the samples of one instance
/// whatever PRESENTATION says ("all the samples belong to a single
/// instance"); the write order across instances shows on the plain take
/// (`topic_scope_ordered_access_keeps_write_order_on_the_plain_take`). Under
/// TOPIC or GROUP scope with `ordered_access` it returned the samples of
/// every instance.
#[test]
fn take_next_instance_returns_one_instance_under_ordered_access() {
    for scope in [
        PresentationAccessScope::Topic,
        PresentationAccessScope::Group,
    ] {
        let batch = interleaved_first_batch(scope, true);
        assert!(!batch.is_empty(), "{scope:?}: nothing came back");
        let first_id = batch[0].id;
        assert!(
            batch.iter().all(|t| t.id == first_id),
            "{scope:?}: the batch is more than one instance, got {batch:?}"
        );
        let seqs: Vec<u32> = batch.iter().map(|t| t.seq).collect();
        assert!(
            seqs.windows(2).all(|w| w[0] < w[1]),
            "{scope:?}: the instance's samples out of order, got {seqs:?}"
        );
    }
}

/// Control: the default (INSTANCE scope, `ordered_access = false`) keeps the
/// instance-cycling behaviour — the first call returns one instance's samples,
/// not the topic-wide write order. Without this the test above would pass on
/// an implementation that ignores the QoS entirely.
#[test]
fn without_ordered_access_the_reader_still_cycles_per_instance() {
    let batch = interleaved_first_batch(PresentationAccessScope::Instance, false);
    assert!(
        !batch.is_empty(),
        "the instance-cycling path must deliver something"
    );
    let first_id = batch[0].id;
    assert!(
        batch.iter().all(|t| t.id == first_id),
        "without ordered_access the batch is one instance, got {batch:?}"
    );
    assert!(
        batch.len() < 4,
        "one instance holds two of the four samples, got {}",
        batch.len()
    );
}

/// `ordered_access = true` with INSTANCE scope changes nothing (§2.2.3.6: the
/// policy pair only takes effect above INSTANCE scope).
#[test]
fn ordered_access_under_instance_scope_is_inert() {
    let batch = interleaved_first_batch(PresentationAccessScope::Instance, true);
    let first_id = batch[0].id;
    assert!(batch.iter().all(|t| t.id == first_id));
}

/// Writes the same interleaved `(id, seq)` stream as
/// [`interleaved_first_batch`] and returns what the plain, topic-wide
/// `take_with_info` hands back — the path a subscriber takes when it does not
/// cycle instances (the OMG `dds-rtps` clients' `--take-read`).
fn interleaved_plain_take(scope: PresentationAccessScope, ordered: bool) -> Vec<Tick> {
    let domain = unique_domain();
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");

    let publisher = participant.create_publisher(PublisherQos {
        presentation: presentation(scope, ordered),
        ..PublisherQos::default()
    });
    let subscriber = participant.create_subscriber(SubscriberQos {
        presentation: presentation(scope, ordered),
        ..SubscriberQos::default()
    });
    let topic = participant
        .create_topic::<Tick>("OrderedPlainTicks", TopicQos::default())
        .expect("topic");
    let writer = publisher
        .create_datawriter::<Tick>(
            &topic,
            DataWriterQos {
                history: keep_last_two_per_instance(),
                ..DataWriterQos::default()
            },
        )
        .expect("writer");
    let reader = subscriber
        .create_datareader::<Tick>(
            &topic,
            DataReaderQos {
                history: keep_last_two_per_instance(),
                ..DataReaderQos::default()
            },
        )
        .expect("reader");

    for (id, seq) in [(1u32, 1u32), (2, 2), (1, 3), (2, 4)] {
        writer.write(&Tick { id, seq }).expect("write");
    }
    subscriber.begin_access().expect("begin_access");
    let batch = reader
        .take_with_info()
        .expect("take_with_info")
        .into_iter()
        .map(|s| s.data)
        .collect();
    subscriber.end_access().expect("end_access");
    batch
}

/// §2.2.3.6, INSTANCE scope: "changes to one instance need not be coherent nor
/// ordered with respect to changes to any other instance. In other words,
/// order and coherent changes apply to each instance separately." A subscriber
/// that asks for ordered access at INSTANCE scope therefore presents its
/// samples grouped per instance — that grouping is the observable difference
/// from the TOPIC scope below, which must keep the writer's order across
/// instances. The OMG `dds-rtps` `Test_OrderedAccess_10` runs both subscribers
/// against one publisher and requires them to differ.
#[test]
fn instance_scope_ordered_access_groups_the_plain_take_by_instance() {
    let batch = interleaved_plain_take(PresentationAccessScope::Instance, true);
    assert_eq!(batch.len(), 4, "all four samples are delivered: {batch:?}");
    let ids: Vec<u32> = batch.iter().map(|t| t.id).collect();
    assert!(
        ids == vec![1, 1, 2, 2] || ids == vec![2, 2, 1, 1],
        "INSTANCE scope must group by instance, got {ids:?}"
    );
    // Write order survives inside each group.
    let group_of =
        |id: u32| -> Vec<u32> { batch.iter().filter(|t| t.id == id).map(|t| t.seq).collect() };
    assert_eq!(group_of(1), vec![1, 3]);
    assert_eq!(group_of(2), vec![2, 4]);
}

/// The counterpart: TOPIC scope keeps the write order across instances on the
/// very same call. Without this the test above would also pass on an
/// implementation that groups by instance unconditionally.
#[test]
fn topic_scope_ordered_access_keeps_write_order_on_the_plain_take() {
    let seqs: Vec<u32> = interleaved_plain_take(PresentationAccessScope::Topic, true)
        .iter()
        .map(|t| t.seq)
        .collect();
    assert_eq!(seqs, vec![1, 2, 3, 4]);
}

/// DDS 1.4 §2.2.2.5.3.8 rule 1: "If PRESENTATION access_scope is INSTANCE,
/// then the returned collection is a 'list' where samples belonging to the
/// same data-instance are consecutive" — with the default PRESENTATION too,
/// `ordered_access` FALSE. The plain take interleaved the instances in
/// write order.
#[test]
fn the_default_presentation_groups_the_plain_take_by_instance() {
    let batch = interleaved_plain_take(PresentationAccessScope::Instance, false);
    assert_eq!(batch.len(), 4, "all four samples are delivered: {batch:?}");
    let ids: Vec<u32> = batch.iter().map(|t| t.id).collect();
    assert!(
        ids == vec![1, 1, 2, 2] || ids == vec![2, 2, 1, 1],
        "the samples of an instance are consecutive, got {ids:?}"
    );
}

/// §2.2.2.5.3.8 rule 2: TOPIC scope without `ordered_access` groups too.
#[test]
fn topic_scope_without_ordered_access_groups_the_plain_take() {
    let batch = interleaved_plain_take(PresentationAccessScope::Topic, false);
    assert_eq!(batch.len(), 4, "all four samples are delivered: {batch:?}");
    let ids: Vec<u32> = batch.iter().map(|t| t.id).collect();
    assert!(
        ids == vec![1, 1, 2, 2] || ids == vec![2, 2, 1, 1],
        "the samples of an instance are consecutive, got {ids:?}"
    );
}

/// §2.2.2.5.3.8 rule 3: GROUP scope without `ordered_access` groups too.
#[test]
fn group_scope_without_ordered_access_groups_the_plain_take() {
    let batch = interleaved_plain_take(PresentationAccessScope::Group, false);
    assert_eq!(batch.len(), 4, "all four samples are delivered: {batch:?}");
    let ids: Vec<u32> = batch.iter().map(|t| t.id).collect();
    assert!(
        ids == vec![1, 1, 2, 2] || ids == vec![2, 2, 1, 1],
        "the samples of an instance are consecutive, got {ids:?}"
    );
}

/// §2.2.2.5.3.8 rule 4: "If PRESENTATION access_scope is GROUP and
/// ordered_access is set to TRUE, then the returned collection contains at
/// most one sample." Each take inside `begin_access`/`end_access` returns
/// one sample, and the takes follow the write order across the instances.
#[test]
fn group_scope_ordered_access_takes_one_sample_at_a_time() {
    let domain = unique_domain();
    let participant = DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), isolated_cfg())
        .expect("participant");
    let group = presentation(PresentationAccessScope::Group, true);
    let publisher = participant.create_publisher(PublisherQos {
        presentation: group,
        ..PublisherQos::default()
    });
    let subscriber = participant.create_subscriber(SubscriberQos {
        presentation: group,
        ..SubscriberQos::default()
    });
    let topic = participant
        .create_topic::<Tick>("GroupOrderedTicks", TopicQos::default())
        .expect("topic");
    let writer = publisher
        .create_datawriter::<Tick>(
            &topic,
            DataWriterQos {
                history: keep_last_two_per_instance(),
                ..DataWriterQos::default()
            },
        )
        .expect("writer");
    let reader = subscriber
        .create_datareader::<Tick>(
            &topic,
            DataReaderQos {
                history: keep_last_two_per_instance(),
                ..DataReaderQos::default()
            },
        )
        .expect("reader");
    for (id, seq) in [(1u32, 1u32), (2, 2), (1, 3), (2, 4)] {
        writer.write(&Tick { id, seq }).expect("write");
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut seqs = Vec::new();
    while seqs.len() < 4 && std::time::Instant::now() < deadline {
        subscriber.begin_access().expect("begin_access");
        let batch = reader.take_with_info().expect("take_with_info");
        subscriber.end_access().expect("end_access");
        assert!(batch.len() <= 1, "a take returned {} samples", batch.len());
        seqs.extend(batch.into_iter().map(|s| s.data.seq));
    }
    assert_eq!(seqs, vec![1, 2, 3, 4]);
}
