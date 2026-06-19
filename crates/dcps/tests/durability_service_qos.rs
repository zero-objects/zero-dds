//! WP QoS-Wiring T6 — DurabilityServiceQosPolicy backend hookup.
//!
//! Spec DDS 1.4 §2.2.3.5: with Durability=Transient/Persistent the
//! DataWriter additionally stores samples in a backend. T6 verifies that
//! the backend is set up automatically for transient writers and that
//! samples flow into it on write().

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
        "volatile writer needs no durability backend"
    );
}

#[test]
fn transient_local_writer_has_no_durability_backend() {
    // TransientLocal lives in the writer history cache, no separate service.
    let w = writer_with_durability(DurabilityKind::TransientLocal);
    assert!(
        w.durability_backend().is_none(),
        "TransientLocal uses the writer history, no separate backend"
    );
}

#[test]
fn transient_writer_has_in_memory_backend() {
    let w = writer_with_durability(DurabilityKind::Transient);
    assert!(
        w.durability_backend().is_some(),
        "transient writer must have an in-memory backend"
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
    assert_eq!(samples.len(), 2, "both samples must be in the backend");
}

#[test]
fn volatile_write_does_not_touch_backend() {
    let w = writer_with_durability(DurabilityKind::Volatile);
    let s = KeyedRecord { id: 9, value: 99 };
    w.write(&s).expect("write");
    assert!(
        w.durability_backend().is_none(),
        "volatile write must not trigger a backend"
    );
}

#[test]
fn transient_backend_outlives_writer_cache_keep_last_eviction() {
    // Setup: writer with Transient + KeepLast(1) — the writer cache holds
    // only 1 sample, but the backend persists all of them. Spec §2.2.3.5:
    // the backend is the single source of truth for late-joiner replay
    // with Transient/Persistent (the writer history is trimmed by
    // HistoryDepth).
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

    // 5 samples, different instances.
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
        "backend must hold all 5 samples (KeepLast depth only for the \
         wire cache, not for the backend); got {}",
        samples.len()
    );
}
