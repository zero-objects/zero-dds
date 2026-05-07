//! WP QoS-Wiring T8b — Reader-Side Wire-Lifecycle-Decode.
//!
//! Spec DDS 1.4 §8.2.1.2 + §9.6.3.9: ReliableReader klassifiziert
//! eingehende DATA-Submessages mit `key_flag=true` und PID_STATUS_INFO
//! als Lifecycle-Marker. delivered_to_user_sample konvertiert das in
//! eine `UserSample::Lifecycle`-Variante, die der DCPS-DataReader-Layer
//! per __push_lifecycle in den Tracker fuettert.

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
use zerodds_dcps::sample_info::InstanceStateKind;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};

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

fn mk_pair() -> (
    zerodds_dcps::DataWriter<KeyedRecord>,
    zerodds_dcps::DataReader<KeyedRecord>,
) {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<KeyedRecord>("LifeCT", TopicQos::default())
        .expect("topic");
    let pub_ = p.create_publisher(PublisherQos::default());
    let w = pub_
        .create_datawriter::<KeyedRecord>(&topic, DataWriterQos::default())
        .expect("writer");
    let sub = p.create_subscriber(SubscriberQos::default());
    let r = sub
        .create_datareader::<KeyedRecord>(&topic, DataReaderQos::default())
        .expect("reader");
    (w, r)
}

#[test]
fn reader_push_lifecycle_disposed_sets_instance_state() {
    // Direkt-Test des __push_lifecycle-Pfades, der vom Live-Channel
    // bei Wire-Lifecycle-Sample gerufen wird.
    let (w, r) = mk_pair();
    let s = KeyedRecord { id: 7, value: 42 };
    w.write(&s).expect("write");
    for b in w.__drain_pending() {
        r.__push_raw(b).expect("push raw alive");
    }
    let _ = r.take_with_info().expect("take initial");

    // Lifecycle-Marker mit Disposed-Bit pushen.
    let mut holder = PlainCdr2BeKeyHolder::new();
    s.encode_key_holder_be(&mut holder);
    let key_bytes = holder.as_bytes().to_vec();
    let max = KeyedRecord::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
    let kh = zerodds_dcps::dds_type::compute_key_hash(&key_bytes, max);
    r.__push_lifecycle(kh, key_bytes, InstanceStateKind::NotAliveDisposed)
        .expect("push lifecycle disposed");

    let samples = r.take_with_info().expect("take after dispose");
    assert!(
        samples.iter().any(|s| {
            !s.info.valid_data && s.info.instance_state == InstanceStateKind::NotAliveDisposed
        }),
        "Reader muss disposed-marker mit valid_data=false sehen, got {samples:?}"
    );
}

#[test]
fn reader_push_lifecycle_unregistered_sets_instance_state() {
    let (w, r) = mk_pair();
    let s = KeyedRecord { id: 8, value: 100 };
    w.write(&s).expect("write");
    for b in w.__drain_pending() {
        r.__push_raw(b).expect("push raw");
    }
    let _ = r.take_with_info().expect("take initial");

    let mut holder = PlainCdr2BeKeyHolder::new();
    s.encode_key_holder_be(&mut holder);
    let key_bytes = holder.as_bytes().to_vec();
    let max = KeyedRecord::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
    let kh = zerodds_dcps::dds_type::compute_key_hash(&key_bytes, max);
    r.__push_lifecycle(kh, key_bytes, InstanceStateKind::NotAliveNoWriters)
        .expect("push lifecycle unregister");

    let samples = r.take_with_info().expect("take after unregister");
    assert!(
        samples
            .iter()
            .any(|s| s.info.instance_state == InstanceStateKind::NotAliveNoWriters),
        "Reader muss unregister-marker als NoWriters sehen, got {samples:?}"
    );
}
