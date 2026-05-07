//! Smoke-Tests fuer die Flatdata-DCPS-Integration (ADR-0005).
//! Nur aktiv mit `--features flatdata-integration`.

#![cfg(feature = "flatdata-integration")]
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

use std::sync::Arc;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::flatdata_integration::{FlatReaderExt, FlatWriterExt};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_flatdata::{FlatStruct, InMemorySlotAllocator, SlotBackend};

#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(C)]
struct Pose {
    x: f64,
    y: f64,
    z: f64,
}

// SAFETY: Pose ist repr(C) + Copy + 'static, alle Felder sind f64.
unsafe impl FlatStruct for Pose {
    const TYPE_HASH: [u8; 16] = [0xAA; 16];
}

impl DdsType for Pose {
    const TYPE_NAME: &'static str = "test::Pose";
    const HAS_KEY: bool = false;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.x.to_le_bytes());
        out.extend_from_slice(&self.y.to_le_bytes());
        out.extend_from_slice(&self.z.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 24 {
            return Err(DecodeError::Invalid {
                what: "Pose: truncated",
            });
        }
        Ok(Self {
            x: f64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            y: f64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            z: f64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        })
    }

    fn encode_key_holder_be(&self, _: &mut PlainCdr2BeKeyHolder) {}
}

#[test]
fn flat_writer_ext_writes_to_backend() {
    // Setup: offline-Participant mit Pose-Topic.
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseFlat", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = Arc::new(
        pubr.create_datawriter::<Pose>(&topic, DataWriterQos::default())
            .expect("writer"),
    );

    let backend: Arc<dyn SlotBackend> = Arc::new(InMemorySlotAllocator::new(0, 4, Pose::WIRE_SIZE));
    let flat_writer = FlatWriterExt::new(Arc::clone(&writer), Arc::clone(&backend), 0b1);

    let p1 = Pose {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    flat_writer.write_flat(&p1).expect("write_flat");

    // Verify: Backend hat das Sample gespeichert.
    let h = zerodds_flatdata::SlotHandle {
        segment_id: 0,
        slot_index: 0,
    };
    let (header, bytes) = backend.read_slot(h).expect("read_slot");
    assert_eq!(header.sample_size as usize, Pose::WIRE_SIZE);
    assert_eq!(bytes.len(), Pose::WIRE_SIZE);
}

#[test]
fn flat_reader_ext_reads_from_backend() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseFlatR", TopicQos::default())
        .expect("topic");

    // Writer + Reader auf gleichem Backend.
    let backend: Arc<dyn SlotBackend> = Arc::new(InMemorySlotAllocator::new(0, 4, Pose::WIRE_SIZE));
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = Arc::new(
        pubr.create_datawriter::<Pose>(&topic, DataWriterQos::default())
            .expect("writer"),
    );
    let flat_writer = FlatWriterExt::new(Arc::clone(&writer), Arc::clone(&backend), 0b1);

    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = Arc::new(
        sub.create_datareader::<Pose>(&topic, DataReaderQos::default())
            .expect("reader"),
    );
    let flat_reader = FlatReaderExt::new(Arc::clone(&reader), Arc::clone(&backend), 0);

    let p1 = Pose {
        x: 7.0,
        y: 8.0,
        z: 9.0,
    };
    flat_writer.write_flat(&p1).expect("write_flat");

    let got = flat_reader.read_flat().expect("read_flat").expect("some");
    assert_eq!(got, p1);
}

#[test]
fn flat_reader_rejects_type_hash_mismatch() {
    // Spec §6.1: backend.type_hash() != T::TYPE_HASH → PreconditionNotMet.
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseFlatHash", TopicQos::default())
        .expect("topic");

    // Backend mit FALSCHEM Type-Hash.
    let wrong_hash = [0xBB; 16];
    let backend: Arc<dyn SlotBackend> =
        Arc::new(InMemorySlotAllocator::new(0, 4, Pose::WIRE_SIZE).with_type_hash(wrong_hash));
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = Arc::new(
        sub.create_datareader::<Pose>(&topic, DataReaderQos::default())
            .expect("reader"),
    );
    let flat_reader = FlatReaderExt::new(Arc::clone(&reader), Arc::clone(&backend), 0);

    let res = flat_reader.read_flat();
    assert!(matches!(
        res,
        Err(zerodds_dcps::DdsError::PreconditionNotMet { .. })
    ));
}

#[test]
fn flat_reader_accepts_matching_type_hash() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseFlatHash2", TopicQos::default())
        .expect("topic");

    let backend: Arc<dyn SlotBackend> =
        Arc::new(InMemorySlotAllocator::new(0, 4, Pose::WIRE_SIZE).with_type_hash(Pose::TYPE_HASH));
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = Arc::new(
        sub.create_datareader::<Pose>(&topic, DataReaderQos::default())
            .expect("reader"),
    );
    let flat_reader = FlatReaderExt::new(Arc::clone(&reader), Arc::clone(&backend), 0);

    // Kein Sample da — read_flat liefert Ok(None), nicht Err.
    let res = flat_reader.read_flat();
    assert!(matches!(res, Ok(None)));
}

#[test]
fn flat_reader_does_not_re_read_same_slot() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseFlatNoDup", TopicQos::default())
        .expect("topic");

    let backend: Arc<dyn SlotBackend> = Arc::new(InMemorySlotAllocator::new(0, 4, Pose::WIRE_SIZE));
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = Arc::new(
        pubr.create_datawriter::<Pose>(&topic, DataWriterQos::default())
            .expect("writer"),
    );
    let flat_writer = FlatWriterExt::new(Arc::clone(&writer), Arc::clone(&backend), 0b1);

    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = Arc::new(
        sub.create_datareader::<Pose>(&topic, DataReaderQos::default())
            .expect("reader"),
    );
    let flat_reader = FlatReaderExt::new(Arc::clone(&reader), Arc::clone(&backend), 0);

    flat_writer
        .write_flat(&Pose::default())
        .expect("write_flat");
    let _ = flat_reader.read_flat().expect("first").expect("some");
    let second = flat_reader.read_flat().expect("second");
    assert!(second.is_none());
}

// ============================================================================
// Spec-konforme Direkt-Methoden auf DataWriter/DataReader (Spec §8.1, §9.1)
// ============================================================================

#[test]
fn datawriter_write_flat_uses_configured_backend() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseDirectWrite", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = pubr
        .create_datawriter::<Pose>(&topic, DataWriterQos::default())
        .expect("writer");

    let backend: Arc<dyn SlotBackend> = Arc::new(InMemorySlotAllocator::new(0, 4, Pose::WIRE_SIZE));
    writer.set_flat_backend(Some(Arc::clone(&backend)), 0b1);

    let sample = Pose {
        x: 4.0,
        y: 5.0,
        z: 6.0,
    };
    writer.write_flat(&sample).expect("write_flat");

    // Backend hat das Sample direkt gespeichert.
    let h = zerodds_flatdata::SlotHandle {
        segment_id: 0,
        slot_index: 0,
    };
    let (header, bytes) = backend.read_slot(h).expect("read_slot");
    assert_eq!(header.sample_size as usize, Pose::WIRE_SIZE);
    assert_eq!(bytes.len(), Pose::WIRE_SIZE);
}

#[test]
fn datawriter_write_flat_falls_back_to_udp_without_backend() {
    // Ohne `set_flat_backend(Some(_))` muss `write_flat` aequivalent
    // zu `write` sein (Spec §4.2: Cross-Host-Fallback ist immer aktiv).
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseFallback", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = pubr
        .create_datawriter::<Pose>(&topic, DataWriterQos::default())
        .expect("writer");

    // Kein Backend → write_flat darf nicht panicen.
    writer
        .write_flat(&Pose {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        })
        .expect("write_flat fallback");
}

#[test]
fn datareader_read_flat_uses_configured_backend() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseDirectRead", TopicQos::default())
        .expect("topic");

    let backend: Arc<dyn SlotBackend> = Arc::new(InMemorySlotAllocator::new(0, 4, Pose::WIRE_SIZE));

    let pubr = p.create_publisher(PublisherQos::default());
    let writer = pubr
        .create_datawriter::<Pose>(&topic, DataWriterQos::default())
        .expect("writer");
    writer.set_flat_backend(Some(Arc::clone(&backend)), 0b1);

    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Pose>(&topic, DataReaderQos::default())
        .expect("reader");
    reader.set_flat_backend(Some(Arc::clone(&backend)), 0);

    let sample = Pose {
        x: 11.0,
        y: 12.0,
        z: 13.0,
    };
    writer.write_flat(&sample).expect("write_flat");

    let got = reader.read_flat().expect("read_flat").expect("some");
    assert_eq!(got, sample);
}

#[test]
fn datareader_read_flat_returns_none_without_backend() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseReadNoBackend", TopicQos::default())
        .expect("topic");
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Pose>(&topic, DataReaderQos::default())
        .expect("reader");

    // Kein Backend gesetzt → read_flat liefert Ok(None) (Caller faellt
    // auf take() zurueck).
    let res = reader.read_flat().expect("read_flat");
    assert!(res.is_none());
}

#[test]
fn datareader_read_flat_rejects_type_hash_mismatch() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseDirectHash", TopicQos::default())
        .expect("topic");

    let wrong_hash = [0xBB; 16];
    let backend: Arc<dyn SlotBackend> =
        Arc::new(InMemorySlotAllocator::new(0, 4, Pose::WIRE_SIZE).with_type_hash(wrong_hash));
    let sub = p.create_subscriber(SubscriberQos::default());
    let reader = sub
        .create_datareader::<Pose>(&topic, DataReaderQos::default())
        .expect("reader");
    reader.set_flat_backend(Some(Arc::clone(&backend)), 0);

    let res = reader.read_flat();
    assert!(matches!(
        res,
        Err(zerodds_dcps::DdsError::PreconditionNotMet { .. })
    ));
}

#[test]
fn datawriter_set_flat_backend_none_disables_shm_path() {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Pose>("PoseDisable", TopicQos::default())
        .expect("topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let writer = pubr
        .create_datawriter::<Pose>(&topic, DataWriterQos::default())
        .expect("writer");

    let backend: Arc<dyn SlotBackend> = Arc::new(InMemorySlotAllocator::new(0, 4, Pose::WIRE_SIZE));
    writer.set_flat_backend(Some(Arc::clone(&backend)), 0b1);
    writer
        .write_flat(&Pose {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        })
        .expect("write_flat with backend");

    // Slot 0 wurde belegt.
    let h0 = zerodds_flatdata::SlotHandle {
        segment_id: 0,
        slot_index: 0,
    };
    let (header_after_first, _) = backend.read_slot(h0).expect("read");
    assert_eq!(header_after_first.sample_size as usize, Pose::WIRE_SIZE);

    // Backend deaktivieren — naechster write_flat darf nichts mehr ins
    // Backend schreiben (kein neuer Slot belegt).
    writer.set_flat_backend(None, 0);
    writer
        .write_flat(&Pose {
            x: 9.0,
            y: 9.0,
            z: 9.0,
        })
        .expect("write_flat without backend");
    let h1 = zerodds_flatdata::SlotHandle {
        segment_id: 0,
        slot_index: 1,
    };
    let (header_slot1, _) = backend.read_slot(h1).expect("read slot1");
    assert_eq!(
        header_slot1.sample_size, 0,
        "Slot 1 darf nicht belegt sein wenn Backend deaktiviert"
    );
}
