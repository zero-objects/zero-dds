//! DDS 1.4 deletion and lookup of the DCPS entities:
//! `DomainParticipant::delete_publisher`/`delete_subscriber`/
//! `delete_contained_entities` (§2.2.2.2.1.2, .4, .18),
//! `Publisher::delete_datawriter`/`lookup_datawriter`/
//! `delete_contained_entities` (§2.2.2.4.1.6, .7, .14) and
//! `Subscriber::delete_datareader`/`lookup_datareader`/`get_datareaders`/
//! `notify_datareaders`/`delete_contained_entities` (§2.2.2.5.2.6, .7,
//! .10, .11, .14). An operation on a deleted entity returns
//! ALREADY_DELETED (§2.2.1.1).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::listener::{DataReaderListener, SubscriberListener};
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::qos::{
    HistoryKind, HistoryQosPolicy, PresentationQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, InstanceHandle, InstanceStateKind, PublisherQos, RawBytes, SubscriberQos,
    TopicQos, instance_state_mask, sample_state_mask, view_state_mask,
};

fn offline(domain: i32) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(domain, DomainParticipantQos::default())
}

fn keep_all_reader() -> DataReaderQos {
    DataReaderQos {
        history: HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

fn already_deleted<T: core::fmt::Debug>(r: Result<T, DdsError>) -> bool {
    matches!(r, Err(DdsError::AlreadyDeleted))
}

fn precondition<T: core::fmt::Debug>(r: Result<T, DdsError>) -> bool {
    matches!(r, Err(DdsError::PreconditionNotMet { .. }))
}

// ---------------------------------------------------------------------------
// Publisher and DataWriter
// ---------------------------------------------------------------------------

#[test]
fn delete_datawriter_deletes_the_writer_behind_every_handle() {
    let p = offline(201);
    let topic = p
        .create_topic::<RawBytes>("DeleteWriter", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let other = writer.clone();
    let handle = writer.instance_handle();
    assert!(publisher.contains_writer(handle));
    assert!(p.contains_entity(handle));

    publisher.delete_datawriter(&writer).unwrap();
    assert!(already_deleted(writer.write(&RawBytes::new(vec![1]))));
    assert!(already_deleted(other.write(&RawBytes::new(vec![1]))));
    assert!(already_deleted(publisher.delete_datawriter(&other)));
    assert!(!publisher.contains_writer(handle));
    assert!(!p.contains_entity(handle));
    assert!(
        publisher
            .lookup_datawriter::<RawBytes>("DeleteWriter")
            .is_none()
    );
    assert!(!publisher.has_datawriters());
    // A deleted writer is no user of its topic.
    p.delete_topic(&topic).unwrap();
}

#[test]
fn a_writer_of_another_publisher_is_left_as_it_was() {
    let p = offline(202);
    let topic = p
        .create_topic::<RawBytes>("ForeignWriter", TopicQos::default())
        .unwrap();
    let mine = p.create_publisher(PublisherQos::default());
    let theirs = p.create_publisher(PublisherQos::default());
    let writer = theirs
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    assert!(precondition(mine.delete_datawriter(&writer)));
    writer.write(&RawBytes::new(vec![7])).unwrap();
    assert!(theirs.contains_writer(writer.instance_handle()));
}

#[test]
fn lookup_datawriter_hands_out_a_handle_to_the_same_writer() {
    let p = offline(203);
    let topic = p
        .create_topic::<RawBytes>("LookupWriter", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    assert!(
        publisher
            .lookup_datawriter::<RawBytes>("NoSuchTopic")
            .is_none()
    );
    let found = publisher
        .lookup_datawriter::<RawBytes>("LookupWriter")
        .expect("the writer");
    assert_eq!(found.instance_handle(), writer.instance_handle());

    found.write(&RawBytes::new(vec![4])).unwrap();
    assert_eq!(writer.__drain_pending().len(), 1, "one writer, one queue");

    // The writer lives while one handle does.
    drop(writer);
    found.write(&RawBytes::new(vec![5])).unwrap();
    assert!(publisher.has_datawriters());
}

#[test]
fn the_last_handle_deletes_the_writer() {
    let p = offline(204);
    let topic = p
        .create_topic::<RawBytes>("LastWriterHandle", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let handle = writer.instance_handle();
    drop(writer);
    assert!(!publisher.contains_writer(handle));
    assert!(
        publisher
            .lookup_datawriter::<RawBytes>("LastWriterHandle")
            .is_none()
    );
    p.delete_publisher(&publisher).unwrap();
}

#[test]
fn publisher_delete_contained_entities_deletes_every_writer() {
    let p = offline(205);
    let a = p
        .create_topic::<RawBytes>("PubContainedA", TopicQos::default())
        .unwrap();
    let b = p
        .create_topic::<RawBytes>("PubContainedB", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let wa = publisher
        .create_datawriter::<RawBytes>(&a, DataWriterQos::default())
        .unwrap();
    let wb = publisher
        .create_datawriter::<RawBytes>(&b, DataWriterQos::default())
        .unwrap();
    publisher.delete_contained_entities().unwrap();
    assert!(already_deleted(wa.write(&RawBytes::new(vec![1]))));
    assert!(already_deleted(wb.write(&RawBytes::new(vec![1]))));
    assert!(!publisher.has_datawriters());
    p.delete_publisher(&publisher).unwrap();
}

#[test]
fn delete_publisher_refuses_a_publisher_with_writers() {
    let p = offline(206);
    let topic = p
        .create_topic::<RawBytes>("PublisherWithWriter", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    assert_eq!(p.publishers_len(), 1);
    assert!(precondition(p.delete_publisher(&publisher)));
    writer.write(&RawBytes::new(vec![1])).unwrap();

    publisher.delete_datawriter(&writer).unwrap();
    p.delete_publisher(&publisher).unwrap();
    assert_eq!(p.publishers_len(), 0);
    assert!(!p.contains_entity(zerodds_dcps::Entity::get_instance_handle(&publisher)));
    assert!(already_deleted(publisher.create_datawriter::<RawBytes>(
        &topic,
        DataWriterQos::default()
    )));
    assert!(already_deleted(p.delete_publisher(&publisher)));
}

#[test]
fn delete_publisher_refuses_a_publisher_of_another_participant() {
    let p = offline(207);
    let q = offline(208);
    let theirs = q.create_publisher(PublisherQos::default());
    assert!(precondition(p.delete_publisher(&theirs)));
    q.delete_publisher(&theirs).unwrap();
}

// ---------------------------------------------------------------------------
// Subscriber and DataReader
// ---------------------------------------------------------------------------

#[test]
fn delete_datareader_refuses_a_reader_that_holds_conditions() {
    let p = offline(209);
    let topic = p
        .create_topic::<RawBytes>("ReaderWithCondition", TopicQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, keep_all_reader())
        .unwrap();
    let condition = reader
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    assert!(precondition(subscriber.delete_datareader(&reader)));
    reader.__push_raw(vec![1]).unwrap();
    assert_eq!(
        reader.take().unwrap().len(),
        1,
        "the reader was left as it was"
    );

    reader.delete_readcondition(&condition).unwrap();
    subscriber.delete_datareader(&reader).unwrap();
    assert!(already_deleted(reader.take()));
    assert!(already_deleted(reader.__push_raw(vec![2])));
    assert!(!subscriber.contains_reader(reader.instance_handle()));
}

#[test]
fn a_reader_of_another_subscriber_is_left_as_it_was() {
    let p = offline(210);
    let topic = p
        .create_topic::<RawBytes>("ForeignReader", TopicQos::default())
        .unwrap();
    let mine = p.create_subscriber(SubscriberQos::default());
    let theirs = p.create_subscriber(SubscriberQos::default());
    let reader = theirs
        .create_datareader::<RawBytes>(&topic, keep_all_reader())
        .unwrap();
    assert!(precondition(mine.delete_datareader(&reader)));
    reader.__push_raw(vec![3]).unwrap();
    assert_eq!(reader.take().unwrap().len(), 1);
}

#[test]
fn lookup_datareader_hands_out_a_handle_to_the_same_reader() {
    let p = offline(211);
    let topic = p
        .create_topic::<RawBytes>("LookupReader", TopicQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, keep_all_reader())
        .unwrap();
    assert!(
        subscriber
            .lookup_datareader::<RawBytes>("NoSuchTopic")
            .is_none()
    );
    let found = subscriber
        .lookup_datareader::<RawBytes>("LookupReader")
        .expect("the reader");
    assert_eq!(found.instance_handle(), reader.instance_handle());
    reader.__push_raw(vec![8]).unwrap();
    assert_eq!(found.take().unwrap().len(), 1, "one reader, one cache");
    assert!(reader.take().unwrap().is_empty());
}

#[test]
fn get_datareaders_names_the_readers_that_hold_samples_in_the_states() {
    let p = offline(212);
    let a = p
        .create_topic::<RawBytes>("GetReadersA", TopicQos::default())
        .unwrap();
    let b = p
        .create_topic::<RawBytes>("GetReadersB", TopicQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let ra = subscriber
        .create_datareader::<RawBytes>(&a, keep_all_reader())
        .unwrap();
    let _rb = subscriber
        .create_datareader::<RawBytes>(&b, keep_all_reader())
        .unwrap();
    let not_read = |s: &zerodds_dcps::Subscriber| {
        s.get_datareaders(
            sample_state_mask::NOT_READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap()
    };
    assert!(not_read(&subscriber).is_empty());

    ra.__push_raw(vec![1]).unwrap();
    let readers = not_read(&subscriber);
    assert_eq!(readers.len(), 1);
    assert_eq!(readers[0].instance_handle(), ra.instance_handle());
    assert_eq!(readers[0].topic_name(), "GetReadersA");
    let typed = readers[0].typed::<RawBytes>().expect("a RawBytes reader");
    assert!(readers[0].typed::<ShapeType>().is_none());

    assert_eq!(typed.read().unwrap().len(), 1);
    assert!(not_read(&subscriber).is_empty(), "the sample is READ now");
    let read = subscriber
        .get_datareaders(
            sample_state_mask::READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    assert_eq!(read.len(), 1);
}

fn group_subscriber(p: &DomainParticipant, ordered: bool) -> zerodds_dcps::Subscriber {
    p.create_subscriber(SubscriberQos {
        presentation: PresentationQosPolicy {
            access_scope: zerodds_qos::PresentationAccessScope::Group,
            coherent_access: false,
            ordered_access: ordered,
        },
        ..SubscriberQos::default()
    })
}

#[test]
fn under_group_scope_get_datareaders_needs_begin_access() {
    let p = offline(213);
    let topic = p
        .create_topic::<RawBytes>("GroupAccess", TopicQos::default())
        .unwrap();
    let subscriber = group_subscriber(&p, false);
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, keep_all_reader())
        .unwrap();
    reader.__push_raw(vec![1]).unwrap();
    let all = || {
        subscriber.get_datareaders(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
    };
    assert!(precondition(all()));
    subscriber.begin_access().expect("begin_access");
    assert_eq!(all().unwrap().len(), 1);
    subscriber.end_access().unwrap();
}

#[test]
fn group_ordered_access_lists_a_reader_once_per_sample_in_write_order() {
    let p = offline(214);
    let a = p
        .create_topic::<RawBytes>("OrderedA", TopicQos::default())
        .unwrap();
    let b = p
        .create_topic::<RawBytes>("OrderedB", TopicQos::default())
        .unwrap();
    let subscriber = group_subscriber(&p, true);
    let ra = subscriber
        .create_datareader::<RawBytes>(&a, keep_all_reader())
        .unwrap();
    let rb = subscriber
        .create_datareader::<RawBytes>(&b, keep_all_reader())
        .unwrap();
    // A sample pushed without a source timestamp takes the time it enters
    // the reader's cache; a read right after the push gives each its own.
    subscriber.begin_access().expect("begin_access");
    for (reader, byte) in [(&ra, 1u8), (&rb, 2), (&ra, 3)] {
        reader.__push_raw(vec![byte]).unwrap();
        reader.read().unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    let list: Vec<InstanceHandle> = subscriber
        .get_datareaders(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap()
        .iter()
        .map(zerodds_dcps::AnyDataReader::instance_handle)
        .collect();
    subscriber.end_access().unwrap();
    assert_eq!(
        list,
        vec![
            ra.instance_handle(),
            rb.instance_handle(),
            ra.instance_handle()
        ]
    );
}

/// DDS 1.4 §2.2.2.5.2.10 with GROUP and `ordered_access`: the readers come
/// "in the order in which the samples were written". For writers of one
/// GROUP-scope publisher that is the group sequence number (DDSI-RTPS 2.5
/// §8.7.5), not the source timestamp: here the sample with GSN 1 carries the
/// later timestamp and its reader comes first.
#[test]
fn group_ordered_access_orders_the_readers_by_group_sequence_number() {
    use zerodds_dcps::runtime::{SampleGroup, UserSample};
    use zerodds_dcps::time::{Time, time_to_he_timestamp};
    use zerodds_rtps::group_digest::GroupDigest;
    use zerodds_rtps::writer_group::ChangeGroupInfo;

    let p = offline(220);
    let a = p
        .create_topic::<RawBytes>("GsnA", TopicQos::default())
        .unwrap();
    let b = p
        .create_topic::<RawBytes>("GsnB", TopicQos::default())
        .unwrap();
    let subscriber = group_subscriber(&p, true);
    let ra = subscriber
        .create_datareader::<RawBytes>(&a, keep_all_reader())
        .unwrap();
    let rb = subscriber
        .create_datareader::<RawBytes>(&b, keep_all_reader())
        .unwrap();
    let push = |reader: &zerodds_dcps::DataReader<RawBytes>, gsn: i64, seconds: i32| {
        let guid = [gsn as u8; 16];
        reader
            .__inbox_handle()
            .lock()
            .unwrap()
            .push(UserSample::Alive {
                key_hash: None,
                payload: zerodds_dcps::sample_bytes::SampleBytes::from_vec(vec![gsn as u8]),
                writer_guid: guid,
                writer_strength: 0,
                representation: 0,
                big_endian: false,
                source_timestamp: Some(time_to_he_timestamp(Time::new(seconds, 0))),
                reception_timestamp: None,
                source_sequence_number: -1,
                coherent_set_sn: None,
                lifespan_nanos: 0,
                related_sample_identity: None,
                group: Some(SampleGroup {
                    writer: guid,
                    info: ChangeGroupInfo {
                        gsn,
                        writer_set: GroupDigest([1, 2, 3, 4]),
                        coherent_set_gsn: None,
                    },
                }),
                crypto_content: false,
            });
    };
    // Reader A holds GSN 2 at 10 s, reader B GSN 1 at 20 s.
    push(&ra, 2, 10);
    push(&rb, 1, 20);
    subscriber.begin_access().expect("begin_access");
    let list: Vec<InstanceHandle> = subscriber
        .get_datareaders(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap()
        .iter()
        .map(zerodds_dcps::AnyDataReader::instance_handle)
        .collect();
    subscriber.end_access().unwrap();
    assert_eq!(list, vec![rb.instance_handle(), ra.instance_handle()]);
}

#[derive(Default)]
struct OnReaders(AtomicU32);
impl SubscriberListener for OnReaders {
    fn on_data_on_readers(&self, _subscriber: InstanceHandle) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct OnDataAvailable(AtomicU32);
impl DataReaderListener for OnDataAvailable {
    fn on_data_available(&self, _reader: InstanceHandle) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn notify_datareaders_calls_the_readers_listeners_once_per_change() {
    let p = offline(215);
    let topic = p
        .create_topic::<RawBytes>("NotifyReaders", TopicQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, keep_all_reader())
        .unwrap();
    let on_readers = Arc::new(OnReaders::default());
    let available = Arc::new(OnDataAvailable::default());
    subscriber.set_listener(Some(on_readers.clone()), status::DATA_ON_READERS);
    reader.set_listener(Some(available.clone()), status::DATA_AVAILABLE);

    reader.__push_raw(vec![1]).unwrap();
    assert_eq!(on_readers.0.load(Ordering::SeqCst), 1);
    assert_eq!(available.0.load(Ordering::SeqCst), 0, "§2.2.4.4");
    subscriber.notify_datareaders().unwrap();
    assert_eq!(available.0.load(Ordering::SeqCst), 1);
    subscriber.notify_datareaders().unwrap();
    assert_eq!(available.0.load(Ordering::SeqCst), 1, "one call per change");

    // A take ends the change before a notify.
    reader.__push_raw(vec![2]).unwrap();
    assert_eq!(reader.take().unwrap().len(), 2);
    subscriber.notify_datareaders().unwrap();
    assert_eq!(available.0.load(Ordering::SeqCst), 1);
}

#[test]
fn subscriber_delete_contained_entities_deletes_readers_and_their_conditions() {
    let p = offline(216);
    let topic = p
        .create_topic::<RawBytes>("SubContained", TopicQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, keep_all_reader())
        .unwrap();
    let condition = reader
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    reader.__push_raw(vec![1]).unwrap();
    assert!(zerodds_dcps::Condition::get_trigger_value(&*condition));

    subscriber.delete_contained_entities().unwrap();
    assert!(!zerodds_dcps::Condition::get_trigger_value(&*condition));
    assert!(already_deleted(reader.take()));
    assert!(!subscriber.has_datareaders());
    p.delete_subscriber(&subscriber).unwrap();
    assert!(already_deleted(
        subscriber.create_datareader::<RawBytes>(&topic, keep_all_reader())
    ));
}

#[test]
fn delete_subscriber_refuses_a_subscriber_with_readers_or_of_another_participant() {
    let p = offline(217);
    let q = offline(218);
    let topic = p
        .create_topic::<RawBytes>("SubscriberWithReader", TopicQos::default())
        .unwrap();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, keep_all_reader())
        .unwrap();
    assert!(precondition(p.delete_subscriber(&subscriber)));
    assert!(precondition(q.delete_subscriber(&subscriber)));
    subscriber.delete_datareader(&reader).unwrap();
    p.delete_subscriber(&subscriber).unwrap();
    assert_eq!(p.subscribers_len(), 0);
}

#[test]
fn participant_delete_contained_entities_deletes_everything_it_created() {
    let p = offline(219);
    let topic = p
        .create_topic::<RawBytes>("ParticipantContained", TopicQos::default())
        .unwrap();
    let publisher = p.create_publisher(PublisherQos::default());
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .unwrap();
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, keep_all_reader())
        .unwrap();
    let condition = reader
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();

    p.delete_contained_entities().unwrap();
    assert!(already_deleted(writer.write(&RawBytes::new(vec![1]))));
    assert!(already_deleted(reader.take()));
    assert!(already_deleted(reader.delete_readcondition(&condition)));
    assert!(already_deleted(publisher.create_datawriter::<RawBytes>(
        &topic,
        DataWriterQos::default()
    )));
    assert!(already_deleted(
        subscriber.create_datareader::<RawBytes>(&topic, keep_all_reader())
    ));
    assert_eq!(p.publishers_len(), 0);
    assert_eq!(p.subscribers_len(), 0);
    assert_eq!(p.topics_len(), 0);
    assert!(!p.contains_entity(writer.instance_handle()));
    assert!(!p.contains_entity(reader.instance_handle()));
}

// ---------------------------------------------------------------------------
// On the wire: a deleted writer unregisters and disposes its instances
// ---------------------------------------------------------------------------

#[test]
fn delete_datawriter_disposes_and_unregisters_its_instances() {
    let lease = common::lease_domain();
    let factory = DomainParticipantFactory::instance();
    let participant = common::ParticipantGuard::new(
        factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                common::isolated_cfg(),
            )
            .expect("participant"),
    );
    let topic = participant
        .create_topic::<ShapeType>("DeleteWriterInstances", TopicQos::default())
        .unwrap();
    let reliable = ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        ..Default::default()
    };
    let publisher = participant.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<ShapeType>(
            &topic,
            DataWriterQos {
                reliability: reliable,
                ..Default::default()
            },
        )
        .unwrap();
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<ShapeType>(
            &topic,
            DataReaderQos {
                reliability: reliable,
                ..keep_all_reader()
            },
        )
        .unwrap();
    writer
        .wait_for_matched_subscription(1, common::match_timeout())
        .expect("match");
    let shape = |color: &str| ShapeType::new(color, 1, 2, 30);
    writer.write(&shape("RED")).unwrap();
    writer.write(&shape("BLUE")).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let mut alive = 0;
    while alive < 2 && Instant::now() < until {
        alive += reader.take().unwrap().len();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(alive, 2);

    publisher.delete_datawriter(&writer).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let mut disposed = std::collections::BTreeSet::new();
    while disposed.len() < 2 && Instant::now() < until {
        for sample in reader.take_with_info().unwrap() {
            if sample.info.instance_state == InstanceStateKind::NotAliveDisposed {
                disposed.insert(sample.info.instance_handle);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        disposed.len(),
        2,
        "both instances disposed by the deletion (autodispose_unregistered_instances)"
    );
}
