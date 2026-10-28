//! DDSI-RTPS 2.5 §8.7.5, §8.7.6 — a publisher and a subscriber with
//! `PRESENTATION.access_scope = GROUP`.
//!
//! §8.7.5: the subscriber commits a sample of a GROUP-scope publisher's
//! writer once the group sequence number before it is committed, or once
//! no matched writer of a reliable reader has it. A writer this subscriber
//! does not read leaves numbers it never sees; the commit rule passes them
//! on the matched writers' own numbers, not after a timeout.
//!
//! §8.7.6: a group coherent set spans the publisher's writers, and the
//! subscriber presents it in all its readers at once, after every writer's
//! end-of-coherent-set DATA.
//!
//! A dispose is a change of its writer like a sample: it has a group
//! sequence number of its own, and between `begin_coherent_changes` and
//! `end_coherent_changes` it is one of the set's changes, at GROUP scope
//! and at TOPIC scope alike.
//!
//! Each case runs on the same-runtime route and between two participants
//! over UDP.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs
)]

#[path = "common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::qos::{
    HistoryKind, HistoryQosPolicy, PresentationAccessScope, PresentationQosPolicy, ReliabilityKind,
    ReliabilityQosPolicy,
};
use zerodds_dcps::{
    DataReader, DataReaderQos, DataWriter, DataWriterQos, DdsError, DdsType, DomainParticipant,
    DomainParticipantFactory, DomainParticipantQos, HANDLE_NIL, Publisher, PublisherQos, RawBytes,
    Subscriber, SubscriberQos, TopicQos,
};

/// `id` is the instance key, `value` what the test reads back.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Keyed {
    id: u32,
    value: u32,
}

impl DdsType for Keyed {
    const TYPE_NAME: &'static str = "test::GroupKeyed";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_be_bytes());
        out.extend_from_slice(&self.value.to_be_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let word = |at: usize| -> Result<u32, DecodeError> {
            bytes
                .get(at..at + 4)
                .and_then(|b| b.try_into().ok())
                .map(u32::from_be_bytes)
                .ok_or(DecodeError::Invalid {
                    what: "truncated Keyed",
                })
        };
        Ok(Self {
            id: word(0)?,
            value: word(4).unwrap_or(0),
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let head: [u8; 4] =
            bytes
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .ok_or(DecodeError::Invalid {
                    what: "truncated Keyed key holder",
                })?;
        Ok(Self {
            id: u32::from_be_bytes(head),
            value: 0,
        })
    }
}

fn keyed(id: u32, value: u32) -> Keyed {
    Keyed { id, value }
}

use common::{ParticipantGuard, isolated_cfg, match_timeout, unique_domain};

fn group(ordered: bool, coherent: bool) -> PresentationQosPolicy {
    PresentationQosPolicy {
        access_scope: PresentationAccessScope::Group,
        coherent_access: coherent,
        ordered_access: ordered,
    }
}

fn topic_coherent() -> PresentationQosPolicy {
    PresentationQosPolicy {
        access_scope: PresentationAccessScope::Topic,
        coherent_access: true,
        ordered_access: false,
    }
}

fn reliable_keep_all_reader() -> DataReaderQos {
    DataReaderQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            ..ReliabilityQosPolicy::default()
        },
        ..DataReaderQos::default()
    }
}

fn reliable_keep_all_writer() -> DataWriterQos {
    DataWriterQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        reliability: ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            ..ReliabilityQosPolicy::default()
        },
        ..DataWriterQos::default()
    }
}

fn sample(n: u8) -> RawBytes {
    RawBytes::new(vec![n])
}

/// The first octet of every sample `reader` takes until it has `n` or
/// `within` passed.
/// `f` on `reader` inside a `begin_access`/`end_access` bracket of its
/// Subscriber: DDS 1.4 §2.2.2.5.2.8 makes a read or take under GROUP access
/// scope PRECONDITION_NOT_MET outside one.
fn accessed<X: zerodds_dcps::DdsType + Send + 'static, R>(
    reader: &DataReader<X>,
    f: impl FnOnce(&DataReader<X>) -> R,
) -> R {
    let subscriber = reader.get_subscriber();
    subscriber.begin_access().expect("begin_access");
    let out = f(reader);
    subscriber.end_access().expect("end_access");
    out
}

fn take_until(reader: &DataReader<RawBytes>, n: usize, within: Duration) -> Vec<u8> {
    let deadline = Instant::now() + within;
    let mut got = Vec::new();
    while got.len() < n && Instant::now() < deadline {
        for s in accessed(reader, |r| r.take()).expect("take") {
            got.push(s.data[0]);
        }
        if got.len() < n {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    got
}

/// A participant pair: one participant for both sides, or two that find
/// each other over UDP.
struct Sides {
    publishing: ParticipantGuard,
    subscribing: Option<ParticipantGuard>,
}

impl Sides {
    fn new(two: bool) -> Self {
        let domain = unique_domain();
        let factory = DomainParticipantFactory::instance();
        let cfg = isolated_cfg();
        let publishing = ParticipantGuard::new(
            factory
                .create_participant_with_config(
                    domain,
                    DomainParticipantQos::default(),
                    cfg.clone(),
                )
                .expect("participant"),
        );
        let subscribing = two.then(|| {
            ParticipantGuard::new(
                factory
                    .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
                    .expect("second participant"),
            )
        });
        Self {
            publishing,
            subscribing,
        }
    }

    fn sub(&self) -> &DomainParticipant {
        self.subscribing.as_deref().unwrap_or(&self.publishing)
    }
}

/// A GROUP-scope publisher with writers on `topics`, and a GROUP-scope
/// subscriber with readers on `read`.
fn group_pair<T: DdsType + Send + 'static>(
    sides: &Sides,
    presentation: PresentationQosPolicy,
    topics: &[&str],
    read: &[&str],
) -> (
    Publisher,
    Vec<DataWriter<T>>,
    Subscriber,
    Vec<DataReader<T>>,
) {
    let publisher = sides.publishing.create_publisher(PublisherQos {
        presentation,
        ..PublisherQos::default()
    });
    let subscriber = sides.sub().create_subscriber(SubscriberQos {
        presentation,
        ..SubscriberQos::default()
    });
    let writers: Vec<_> = topics
        .iter()
        .map(|name| {
            let topic = sides
                .publishing
                .create_topic::<T>(name, TopicQos::default())
                .expect("topic");
            publisher
                .create_datawriter::<T>(&topic, reliable_keep_all_writer())
                .expect("writer")
        })
        .collect();
    let readers: Vec<_> = read
        .iter()
        .map(|name| {
            let topic = sides
                .sub()
                .create_topic::<T>(name, TopicQos::default())
                .expect("topic");
            subscriber
                .create_datareader::<T>(&topic, reliable_keep_all_reader())
                .expect("reader")
        })
        .collect();
    for (name, writer) in topics.iter().zip(&writers) {
        if read.contains(name) {
            writer
                .wait_for_matched_subscription(1, match_timeout())
                .expect("match");
        }
    }
    (publisher, writers, subscriber, readers)
}

/// The publisher writes on topics A and B alternately; the subscriber reads
/// only B. The group sequence numbers of A's samples never reach it, and
/// the commit rule passes them on B's writer's numbers: the samples arrive
/// without waiting out the one-second timeout.
fn reader_of_one_topic(two: bool, label: &str) {
    let sides = Sides::new(two);
    let a = format!("GroupOrderA{label}");
    let b = format!("GroupOrderB{label}");
    let (_publisher, writers, _subscriber, readers) =
        group_pair(&sides, group(true, false), &[&a, &b], &[&b]);
    let start = Instant::now();
    for n in 1..=3u8 {
        writers[0].write(&sample(10 + n)).expect("write A");
        writers[1].write(&sample(20 + n)).expect("write B");
    }
    let got = take_until(&readers[0], 3, Duration::from_secs(5));
    assert_eq!(got, [21, 22, 23]);
    assert!(
        start.elapsed() < Duration::from_millis(900),
        "{label}: B's samples waited {:?}, the group order's timeout is 1 s",
        start.elapsed()
    );
}

#[test]
fn a_reader_of_one_topic_gets_its_samples_without_the_others_numbers_same_runtime() {
    reader_of_one_topic(false, "Local");
}

#[test]
fn a_reader_of_one_topic_gets_its_samples_without_the_others_numbers_over_udp() {
    reader_of_one_topic(true, "Udp");
}

/// A group coherent set with a sample on topic A and one on topic B: no
/// reader sees anything before `end_coherent_changes`, then each reader its
/// sample.
fn coherent_set_across_topics(two: bool, label: &str) {
    let sides = Sides::new(two);
    let a = format!("GroupSetA{label}");
    let b = format!("GroupSetB{label}");
    let (publisher, writers, _subscriber, readers) =
        group_pair(&sides, group(false, true), &[&a, &b], &[&a, &b]);
    publisher.begin_coherent_changes().expect("begin");
    writers[0].write(&sample(1)).expect("write A");
    writers[1].write(&sample(2)).expect("write B");
    // Long enough for both samples to arrive; none may be visible.
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        accessed(&readers[0], |r| r.take())
            .expect("take A")
            .is_empty(),
        "{label}: A before the end"
    );
    assert!(
        accessed(&readers[1], |r| r.take())
            .expect("take B")
            .is_empty(),
        "{label}: B before the end"
    );
    assert_eq!(
        readers[0].coherent_held_len() + readers[1].coherent_held_len(),
        2,
        "{label}: both members are held, not dropped"
    );
    publisher.end_coherent_changes().expect("end");
    assert_eq!(take_until(&readers[0], 1, Duration::from_secs(5)), [1]);
    assert_eq!(take_until(&readers[1], 1, Duration::from_secs(5)), [2]);
    assert_eq!(
        readers[0].coherent_held_len() + readers[1].coherent_held_len(),
        0
    );
}

#[test]
fn a_group_coherent_set_across_two_topics_appears_after_its_end_same_runtime() {
    coherent_set_across_topics(false, "Local");
}

#[test]
fn a_group_coherent_set_across_two_topics_appears_after_its_end_over_udp() {
    coherent_set_across_topics(true, "Udp");
}

/// The value of each valid sample `reader` takes until it has `n` changes
/// or `within` passed, a dispose as `None`.
fn take_changes(reader: &DataReader<Keyed>, n: usize, within: Duration) -> Vec<Option<u32>> {
    let deadline = Instant::now() + within;
    let mut got = Vec::new();
    while got.len() < n && Instant::now() < deadline {
        for s in accessed(reader, |r| r.take_with_info()).expect("take") {
            got.push(s.info.valid_data.then_some(s.data.value));
        }
        if got.len() < n {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    got
}

/// Two readers of topic B in the subscriber: each gets its own copy of
/// every sample of B's writer, in order, without the one-second timeout.
fn two_readers_of_one_topic(two: bool, label: &str) {
    let sides = Sides::new(two);
    let a = format!("GroupTwoA{label}");
    let b = format!("GroupTwoB{label}");
    let (_publisher, writers, subscriber, readers) =
        group_pair::<RawBytes>(&sides, group(true, false), &[&a, &b], &[&b]);
    let topic = sides
        .sub()
        .create_topic::<RawBytes>(&b, TopicQos::default())
        .expect("topic");
    let second = subscriber
        .create_datareader::<RawBytes>(&topic, reliable_keep_all_reader())
        .expect("second reader");
    writers[1]
        .wait_for_matched_subscription(2, match_timeout())
        .expect("match both");
    let start = Instant::now();
    for n in 1..=3u8 {
        writers[0].write(&sample(10 + n)).expect("write A");
        writers[1].write(&sample(20 + n)).expect("write B");
    }
    assert_eq!(
        take_until(&readers[0], 3, Duration::from_secs(5)),
        [21, 22, 23],
        "{label}"
    );
    assert_eq!(
        take_until(&second, 3, Duration::from_secs(5)),
        [21, 22, 23],
        "{label}"
    );
    assert!(
        start.elapsed() < Duration::from_millis(900),
        "{label}: the samples waited {:?}, the group order's timeout is 1 s",
        start.elapsed()
    );
}

#[test]
fn two_readers_of_one_topic_each_get_every_sample_same_runtime() {
    two_readers_of_one_topic(false, "Local");
}

#[test]
fn two_readers_of_one_topic_each_get_every_sample_over_udp() {
    two_readers_of_one_topic(true, "Udp");
}

/// GROUP order with a dispose between the samples: the dispose has the
/// group sequence number it was given, so B's next sample does not wait
/// for it until the one-second timeout.
fn dispose_in_group_order(two: bool, label: &str) {
    let sides = Sides::new(two);
    let a = format!("GroupDisposeA{label}");
    let b = format!("GroupDisposeB{label}");
    let (_publisher, writers, _subscriber, readers) =
        group_pair::<Keyed>(&sides, group(true, false), &[&a, &b], &[&b]);
    let start = Instant::now();
    writers[0].write(&keyed(1, 11)).expect("write A");
    writers[1].write(&keyed(1, 21)).expect("write B");
    writers[1]
        .dispose(&keyed(1, 0), HANDLE_NIL)
        .expect("dispose B");
    writers[0].write(&keyed(1, 12)).expect("write A");
    writers[1].write(&keyed(1, 22)).expect("write B");
    let got = take_changes(&readers[0], 3, Duration::from_secs(5));
    assert_eq!(got, [Some(21), None, Some(22)], "{label}");
    assert!(
        start.elapsed() < Duration::from_millis(900),
        "{label}: B's changes waited {:?}, the group order's timeout is 1 s",
        start.elapsed()
    );
}

#[test]
fn a_dispose_takes_its_place_in_the_group_order_same_runtime() {
    dispose_in_group_order(false, "Local");
}

#[test]
fn a_dispose_takes_its_place_in_the_group_order_over_udp() {
    dispose_in_group_order(true, "Udp");
}

/// A coherent set with a sample, a dispose and a sample: nothing of it,
/// the dispose included, before `end_coherent_changes`, then all three.
fn dispose_in_coherent_set(presentation: PresentationQosPolicy, two: bool, label: &str) {
    let sides = Sides::new(two);
    let a = format!("SetDispose{label}");
    let (publisher, writers, _subscriber, readers) =
        group_pair::<Keyed>(&sides, presentation, &[&a], &[&a]);
    publisher.begin_coherent_changes().expect("begin");
    writers[0].write(&keyed(1, 1)).expect("write");
    writers[0]
        .dispose(&keyed(1, 0), HANDLE_NIL)
        .expect("dispose");
    writers[0].write(&keyed(2, 2)).expect("write");
    // Long enough for all three to arrive; none may be visible.
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        accessed(&readers[0], |r| r.take_with_info())
            .expect("take")
            .is_empty(),
        "{label}: a change before the end"
    );
    assert_eq!(readers[0].coherent_held_len(), 3, "{label}: all three held");
    publisher.end_coherent_changes().expect("end");
    assert_eq!(
        take_changes(&readers[0], 3, Duration::from_secs(5)),
        [Some(1), None, Some(2)],
        "{label}"
    );
}

#[test]
fn a_dispose_in_a_topic_coherent_set_waits_for_its_end_same_runtime() {
    dispose_in_coherent_set(topic_coherent(), false, "TopicLocal");
}

#[test]
fn a_dispose_in_a_topic_coherent_set_waits_for_its_end_over_udp() {
    dispose_in_coherent_set(topic_coherent(), true, "TopicUdp");
}

#[test]
fn a_dispose_in_a_group_coherent_set_waits_for_its_end_same_runtime() {
    dispose_in_coherent_set(group(false, true), false, "GroupLocal");
}

#[test]
fn a_dispose_in_a_group_coherent_set_waits_for_its_end_over_udp() {
    dispose_in_coherent_set(group(false, true), true, "GroupUdp");
}

/// A reader made while a coherent set is open. Over UDP the writer gives
/// a reader matched inside a set the whole set from its first member; on
/// the same-runtime route the reader has the set's tail only, which is
/// incomplete and discarded (DDSI-RTPS 2.5 §8.7.6). Either way no part of
/// a set is seen.
fn reader_made_inside_a_set(two: bool, label: &str) -> Vec<u8> {
    let sides = Sides::new(two);
    let a = format!("SetJoin{label}");
    let (publisher, writers, subscriber, _none) =
        group_pair::<RawBytes>(&sides, topic_coherent(), &[&a], &[]);
    publisher.begin_coherent_changes().expect("begin");
    writers[0].write(&sample(1)).expect("write");
    let topic = sides
        .sub()
        .create_topic::<RawBytes>(&a, TopicQos::default())
        .expect("topic");
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, reliable_keep_all_reader())
        .expect("reader");
    writers[0]
        .wait_for_matched_subscription(1, match_timeout())
        .expect("match");
    writers[0].write(&sample(2)).expect("write");
    publisher.end_coherent_changes().expect("end");
    publisher.begin_coherent_changes().expect("begin");
    writers[0].write(&sample(3)).expect("write");
    writers[0].write(&sample(4)).expect("write");
    publisher.end_coherent_changes().expect("end");
    let mut got = take_until(&reader, 4, Duration::from_secs(2));
    got.sort_unstable();
    got
}

#[test]
fn a_reader_made_inside_a_set_does_not_get_its_tail_same_runtime() {
    assert_eq!(reader_made_inside_a_set(false, "Local"), [3, 4]);
}

#[test]
fn a_reader_made_inside_a_set_gets_it_whole_over_udp() {
    assert_eq!(reader_made_inside_a_set(true, "Udp"), [1, 2, 3, 4]);
}

/// §8.7.6: "Writers are not allowed to be added or removed from a Publisher/// §8.7.6: "Writers are not allowed to be added or removed from a Publisher
/// from the time that a coherent set begins until after it ends."
#[test]
fn a_writer_is_not_created_or_deleted_while_a_group_coherent_set_is_open() {
    let sides = Sides::new(false);
    let (publisher, writers, _subscriber, _readers) =
        group_pair::<RawBytes>(&sides, group(false, true), &["GroupFixedA"], &[]);
    let topic = sides
        .publishing
        .create_topic::<RawBytes>("GroupFixedB", TopicQos::default())
        .expect("topic");
    publisher.begin_coherent_changes().expect("begin");
    assert!(matches!(
        publisher.create_datawriter::<RawBytes>(&topic, DataWriterQos::default()),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        publisher.delete_datawriter(&writers[0]),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    publisher.end_coherent_changes().expect("end");
    publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("a writer after the set");
    publisher
        .delete_datawriter(&writers[0])
        .expect("a deletion after the set");
}

/// DDS 1.4 §2.2.2.5.2.8: under GROUP access scope "the operation
/// begin_access must be called prior to calling any of the sample-accessing
/// operations ... Otherwise the sample-accessing operations will return the
/// error PRECONDITION_NOT_MET." A take outside the bracket is refused and
/// leaves the sample; inside it the sample comes. Only get_datareaders
/// asked for the bracket.
#[test]
fn a_take_under_group_scope_wants_begin_access() {
    let sides = Sides::new(false);
    let (_publisher, writers, subscriber, readers) = group_pair::<RawBytes>(
        &sides,
        group(false, false),
        &["GroupBracket"],
        &["GroupBracket"],
    );
    writers[0].write(&sample(7)).expect("write");
    std::thread::sleep(Duration::from_millis(100));
    assert!(matches!(
        readers[0].take(),
        Err(zerodds_dcps::DdsError::PreconditionNotMet { .. })
    ));
    assert!(matches!(
        readers[0].read_with_info(),
        Err(zerodds_dcps::DdsError::PreconditionNotMet { .. })
    ));
    subscriber.begin_access().expect("begin_access");
    let got = readers[0].take().expect("take");
    subscriber.end_access().expect("end_access");
    assert_eq!(got.len(), 1, "the refused take left the sample");
}
