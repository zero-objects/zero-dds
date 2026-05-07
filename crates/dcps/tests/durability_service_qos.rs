//! WP QoS-Wiring T6 — DurabilityServiceQosPolicy Backend-Anschluss.
//!
//! Spec DDS 1.4 §2.2.3.5: bei Durability=Transient/Persistent legt der
//! DataWriter Samples zusaetzlich in einem Backend ab. T6 verifiziert,
//! dass das Backend bei Transient-Writers automatisch eingerichtet ist
//! und Samples beim write() reinfliessen.

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

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::{
    DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos, PublisherQos, TopicQos,
};
use zerodds_qos::DurabilityKind;
use zerodds_qos::DurabilityQosPolicy;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct KeyedRecord {
    id: u32,
    value: u32,
}

impl DdsType for KeyedRecord {
    const TYPE_NAME: &'static str = "test::KeyedRecord";
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_be_bytes());
        out.extend_from_slice(&self.value.to_be_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 4 {
            return Err(DecodeError::Invalid {
                what: "truncated KeyedRecord",
            });
        }
        let id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let value = if bytes.len() >= 8 {
            u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]])
        } else {
            0
        };
        Ok(Self { id, value })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
    }
}

fn writer_with_durability(kind: DurabilityKind) -> zerodds_dcps::DataWriter<KeyedRecord> {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<KeyedRecord>("DurT", TopicQos::default())
        .expect("topic");
    let pub_ = p.create_publisher(PublisherQos::default());
    let qos = DataWriterQos {
        durability: DurabilityQosPolicy { kind },
        ..Default::default()
    };
    pub_.create_datawriter::<KeyedRecord>(&topic, qos)
        .expect("writer")
}

#[test]
fn volatile_writer_has_no_durability_backend() {
    let w = writer_with_durability(DurabilityKind::Volatile);
    assert!(
        w.durability_backend().is_none(),
        "Volatile-Writer braucht kein Durability-Backend"
    );
}

#[test]
fn transient_local_writer_has_no_durability_backend() {
    // TransientLocal liegt im Writer-History-Cache, kein separater Service.
    let w = writer_with_durability(DurabilityKind::TransientLocal);
    assert!(
        w.durability_backend().is_none(),
        "TransientLocal nutzt Writer-History, kein separates Backend"
    );
}

#[test]
fn transient_writer_has_in_memory_backend() {
    let w = writer_with_durability(DurabilityKind::Transient);
    assert!(
        w.durability_backend().is_some(),
        "Transient-Writer muss In-Memory-Backend haben"
    );
}

#[test]
fn transient_write_lands_in_backend() {
    let w = writer_with_durability(DurabilityKind::Transient);
    let s = KeyedRecord { id: 1, value: 100 };
    w.write(&s).expect("write");
    let s2 = KeyedRecord { id: 2, value: 200 };
    w.write(&s2).expect("write");

    let backend = w.durability_backend().expect("backend");
    let samples = backend.replay_for_topic("DurT").expect("replay");
    assert_eq!(samples.len(), 2, "beide Samples muessen im Backend sein");
}

#[test]
fn volatile_write_does_not_touch_backend() {
    let w = writer_with_durability(DurabilityKind::Volatile);
    let s = KeyedRecord { id: 9, value: 99 };
    w.write(&s).expect("write");
    assert!(
        w.durability_backend().is_none(),
        "Volatile-Write darf kein Backend triggern"
    );
}

#[test]
fn transient_backend_outlives_writer_cache_keep_last_eviction() {
    // Setup: Writer mit Transient + KeepLast(1) — Writer-Cache haelt nur
    // 1 Sample, aber Backend persistiert alle. Spec §2.2.3.5: das
    // Backend ist die Single-Source-of-Truth fuer Late-Joiner-Replay
    // bei Transient/Persistent (Writer-History wird durch HistoryDepth
    // beschnitten).
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<KeyedRecord>("DurT", TopicQos::default())
        .expect("topic");
    let pub_ = p.create_publisher(PublisherQos::default());
    let qos = DataWriterQos {
        durability: zerodds_qos::DurabilityQosPolicy {
            kind: DurabilityKind::Transient,
        },
        history: zerodds_qos::HistoryQosPolicy {
            kind: zerodds_qos::HistoryKind::KeepLast,
            depth: 1,
        },
        ..Default::default()
    };
    let w = pub_
        .create_datawriter::<KeyedRecord>(&topic, qos)
        .expect("writer");

    // 5 Samples, verschiedene Instanzen.
    for i in 1u32..=5 {
        w.write(&KeyedRecord {
            id: i,
            value: i * 10,
        })
        .expect("write");
    }

    let backend = w.durability_backend().expect("backend");
    let samples = backend.replay_for_topic("DurT").expect("replay");
    assert!(
        samples.len() >= 5,
        "Backend muss alle 5 Samples halten (KeepLast-Depth nur fuer \
         Wire-Cache, nicht fuer Backend); got {}",
        samples.len()
    );
}
