// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A listener operation gets the entity (DDS 1.4 §2.2.4.3, §2.3.3:
//! `on_data_available(in DataReader the_reader)`): a listener shared by
//! several entities finds out which one it is called for and reaches it. The
//! handle form of the operations still works.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::sync::{Arc, Mutex};

use zerodds_dcps::listener::{
    DataReaderListener, DataWriterListener, DomainParticipantListener, SubscriberListener,
    TopicListener,
};
use zerodds_dcps::listener_dispatch::{
    ReaderListenerChain, TopicListenerChain, WriterListenerChain, dispatch_data_available,
    dispatch_data_on_readers, dispatch_inconsistent_topic, dispatch_publication_matched,
    dispatch_sample_lost,
};
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::publisher::AnyDataWriter;
use zerodds_dcps::status::{InconsistentTopicStatus, PublicationMatchedStatus, SampleLostStatus};
use zerodds_dcps::subscriber::AnyDataReader;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, Entity,
    InstanceHandle, PublisherQos, RawBytes, Subscriber, SubscriberQos, TopicDescription,
    TopicDescriptionHandle, TopicQos,
};

/// One listener for everything, written against the entity forms only.
#[derive(Default)]
struct Shared {
    seen: Mutex<Vec<String>>,
}

impl Shared {
    fn note(&self, line: String) {
        self.seen.lock().unwrap().push(line);
    }
    fn lines(&self) -> Vec<String> {
        self.seen.lock().unwrap().clone()
    }
}

impl DataReaderListener for Shared {
    fn on_data_available_entity(&self, reader: &AnyDataReader) {
        // The listener reaches the reader and, by its type, the data.
        let typed = reader.typed::<RawBytes>().expect("a reader of RawBytes");
        self.note(format!(
            "available {} {}",
            reader.topic_name(),
            typed.matched_publication_count()
        ));
    }
    fn on_sample_lost_entity(&self, reader: &AnyDataReader, status: SampleLostStatus) {
        self.note(format!(
            "lost {} {}",
            reader.topic_name(),
            status.total_count
        ));
    }
}

impl SubscriberListener for Shared {
    fn on_data_on_readers_entity(&self, subscriber: &Subscriber) {
        self.note(format!(
            "on_readers {}",
            subscriber.get_listener().is_some()
        ));
    }
}

impl DataWriterListener for Shared {
    fn on_publication_matched_entity(
        &self,
        writer: &AnyDataWriter,
        status: PublicationMatchedStatus,
    ) {
        self.note(format!(
            "matched {} {}",
            writer.topic_name(),
            status.current_count
        ));
    }
}

impl TopicListener for Shared {
    fn on_inconsistent_topic_entity(
        &self,
        topic: &TopicDescriptionHandle,
        status: InconsistentTopicStatus,
    ) {
        self.note(format!(
            "inconsistent {} {}",
            topic.get_name(),
            status.total_count
        ));
    }
}

/// A listener written before the entity forms: it implements the handle
/// forms only.
#[derive(Default)]
struct Legacy {
    handles: Mutex<Vec<InstanceHandle>>,
}

impl DataReaderListener for Legacy {
    fn on_data_available(&self, reader: InstanceHandle) {
        self.handles.lock().unwrap().push(reader);
    }
}

struct Rig {
    participant: zerodds_dcps::DomainParticipant,
    subscriber: Subscriber,
    publisher: zerodds_dcps::Publisher,
    topic_a: zerodds_dcps::Topic<RawBytes>,
    reader_a: zerodds_dcps::DataReader<RawBytes>,
    reader_b: zerodds_dcps::DataReader<RawBytes>,
    writer_a: zerodds_dcps::DataWriter<RawBytes>,
}

fn rig() -> Rig {
    let participant = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic_a = participant
        .create_topic::<RawBytes>("EntityA", TopicQos::default())
        .unwrap();
    let topic_b = participant
        .create_topic::<RawBytes>("EntityB", TopicQos::default())
        .unwrap();
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let publisher = participant.create_publisher(PublisherQos::default());
    let reader_a = subscriber
        .create_datareader::<RawBytes>(&topic_a, DataReaderQos::default())
        .unwrap();
    let reader_b = subscriber
        .create_datareader::<RawBytes>(&topic_b, DataReaderQos::default())
        .unwrap();
    let writer_a = publisher
        .create_datawriter::<RawBytes>(&topic_a, DataWriterQos::default())
        .unwrap();
    Rig {
        participant,
        subscriber,
        publisher,
        topic_a,
        reader_a,
        reader_b,
        writer_a,
    }
}

/// The reader, the subscriber, the writer and the topic arrive as objects,
/// at the level of the entity and at the levels the status bubbles up to.
#[test]
fn a_shared_listener_is_called_with_the_entity() {
    let r = rig();
    let shared = Arc::new(Shared::default());

    let readers = ReaderListenerChain {
        reader: Some((
            Arc::clone(&shared) as Arc<dyn DataReaderListener>,
            status::DATA_AVAILABLE | status::SAMPLE_LOST,
        )),
        subscriber: Some((
            Arc::clone(&shared) as Arc<dyn SubscriberListener>,
            status::DATA_ON_READERS,
        )),
        participant: None,
    };
    dispatch_data_available(&readers, r.reader_a.instance_handle());
    dispatch_data_available(&readers, r.reader_b.instance_handle());
    dispatch_sample_lost(
        &readers,
        r.reader_b.instance_handle(),
        SampleLostStatus {
            total_count: 4,
            total_count_change: 4,
        },
    );
    assert!(dispatch_data_on_readers(
        &readers,
        r.subscriber.get_instance_handle()
    ));

    let writers = WriterListenerChain {
        writer: Some((
            Arc::clone(&shared) as Arc<dyn DataWriterListener>,
            status::PUBLICATION_MATCHED,
        )),
        publisher: None,
        participant: None,
    };
    assert!(dispatch_publication_matched(
        &writers,
        r.writer_a.instance_handle(),
        PublicationMatchedStatus {
            current_count: 2,
            ..Default::default()
        }
    ));

    let topics = TopicListenerChain {
        topic: Some((
            Arc::clone(&shared) as Arc<dyn TopicListener>,
            status::INCONSISTENT_TOPIC,
        )),
        participant: None,
    };
    assert!(dispatch_inconsistent_topic(
        &topics,
        r.topic_a.get_instance_handle(),
        InconsistentTopicStatus {
            total_count: 1,
            total_count_change: 1,
        }
    ));

    assert_eq!(
        shared.lines(),
        vec![
            "available EntityA 0".to_string(),
            "available EntityB 0".to_string(),
            "lost EntityB 4".to_string(),
            "on_readers false".to_string(),
            "matched EntityA 2".to_string(),
            "inconsistent EntityA 1".to_string(),
        ]
    );
    let _ = (&r.participant, &r.publisher);
}

/// The participant's listener, where a status bubbles up to, gets the entity
/// of the reader too.
#[test]
fn the_bubbled_up_call_gets_the_entity() {
    struct Top(Mutex<Vec<String>>);
    impl DomainParticipantListener for Top {
        fn on_data_available_entity(&self, reader: &AnyDataReader) {
            self.0.lock().unwrap().push(reader.topic_name().to_string());
        }
    }
    let r = rig();
    let top = Arc::new(Top(Mutex::new(Vec::new())));
    let chain = ReaderListenerChain {
        reader: None,
        subscriber: None,
        participant: Some((
            Arc::clone(&top) as Arc<dyn DomainParticipantListener>,
            status::DATA_AVAILABLE,
        )),
    };
    dispatch_data_available(&chain, r.reader_a.instance_handle());
    assert_eq!(*top.0.lock().unwrap(), vec!["EntityA".to_string()]);
}

/// A listener that implements the handle form is called as before: with
/// the handle of the entity, and with a handle that names no entity.
#[test]
fn a_listener_of_the_handle_form_is_still_called() {
    let r = rig();
    let legacy = Arc::new(Legacy::default());
    let chain = ReaderListenerChain {
        reader: Some((
            Arc::clone(&legacy) as Arc<dyn DataReaderListener>,
            status::DATA_AVAILABLE,
        )),
        subscriber: None,
        participant: None,
    };
    dispatch_data_available(&chain, r.reader_a.instance_handle());
    let made_up = InstanceHandle::from_raw(0xDEAD_BEEF_0000_0001);
    dispatch_data_available(&chain, made_up);
    assert_eq!(
        *legacy.handles.lock().unwrap(),
        vec![r.reader_a.instance_handle(), made_up]
    );
}
