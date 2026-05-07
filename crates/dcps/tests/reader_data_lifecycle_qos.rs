//! WP QoS-Wiring T3 — ReaderDataLifecycle autopurge.
//!
//! Spec DDS 1.4 §2.2.3.22: NotAliveDisposed / NotAliveNoWriters Instanzen
//! werden nach `autopurge_disposed_samples_delay` /
//! `autopurge_nowriter_samples_delay` aus dem Reader-Cache entfernt.

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

use std::thread;
use std::time::Duration;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    InstanceStateKind, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_qos::Duration as QosDuration;
use zerodds_qos::ReaderDataLifecycleQosPolicy;

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

fn pair_with_rqos(
    rqos: DataReaderQos,
) -> (
    zerodds_dcps::DataWriter<KeyedRecord>,
    zerodds_dcps::DataReader<KeyedRecord>,
) {
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<KeyedRecord>("Apl", TopicQos::default())
        .expect("topic");
    let pub_ = p.create_publisher(PublisherQos::default());
    let w = pub_
        .create_datawriter::<KeyedRecord>(&topic, DataWriterQos::default())
        .expect("writer");
    let sub = p.create_subscriber(SubscriberQos::default());
    let r = sub
        .create_datareader::<KeyedRecord>(&topic, rqos)
        .expect("reader");
    (w, r)
}

fn pump(w: &zerodds_dcps::DataWriter<KeyedRecord>, r: &zerodds_dcps::DataReader<KeyedRecord>) {
    for b in w.__drain_pending() {
        r.__push_raw(b).expect("push");
    }
}

#[test]
fn default_infinite_delay_keeps_disposed_instance() {
    // Default: beide delays = INFINITE → autopurge tut nichts.
    let (w, r) = pair_with_rqos(DataReaderQos::default());
    let s = KeyedRecord { id: 1, value: 10 };
    w.write(&s).expect("write");
    pump(&w, &r);

    // Schreibe Sample, dispose Instanz, ingest erneut.
    let _ = r.take().expect("take initial");

    // Lifecycle-Marker: Disposed.
    let mut holder = PlainCdr2BeKeyHolder::new();
    s.encode_key_holder_be(&mut holder);
    let key_bytes = holder.as_bytes().to_vec();
    let max = KeyedRecord::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
    let kh = zerodds_dcps::dds_type::compute_key_hash(&key_bytes, max);
    r.__push_lifecycle(kh, key_bytes, InstanceStateKind::NotAliveDisposed)
        .expect("push lifecycle");

    thread::sleep(Duration::from_millis(150));
    let _ = r.take().expect("take");

    // Tracker hat die Instanz noch — autopurge mit INFINITE purgt nicht.
    let tracker = r.instance_tracker();
    let handle = r.lookup_instance(&s);
    assert!(
        tracker.get_by_handle(handle).is_some(),
        "INFINITE-Delay darf disposed Instanz NIEMALS purgen"
    );
}

#[test]
fn finite_disposed_delay_purges_after_window() {
    let rqos = DataReaderQos {
        reader_data_lifecycle: ReaderDataLifecycleQosPolicy {
            autopurge_disposed_samples_delay: QosDuration::from_millis(100),
            autopurge_nowriter_samples_delay: QosDuration::INFINITE,
        },
        ..Default::default()
    };
    let (w, r) = pair_with_rqos(rqos);

    let s = KeyedRecord { id: 2, value: 20 };
    w.write(&s).expect("write");
    pump(&w, &r);
    let _ = r.take().expect("take initial");

    let mut holder = PlainCdr2BeKeyHolder::new();
    s.encode_key_holder_be(&mut holder);
    let key_bytes = holder.as_bytes().to_vec();
    let max = KeyedRecord::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
    let kh = zerodds_dcps::dds_type::compute_key_hash(&key_bytes, max);
    r.__push_lifecycle(kh, key_bytes, InstanceStateKind::NotAliveDisposed)
        .expect("push lifecycle");
    let _ = r.take().expect("take dispose marker");

    // 200ms warten — Delay 100ms ist abgelaufen.
    thread::sleep(Duration::from_millis(200));
    // Trigger des autopurge-Hooks via take().
    let _ = r.take().expect("take after purge window");

    let tracker = r.instance_tracker();
    let handle = r.lookup_instance(&s);
    assert!(
        tracker.get_by_handle(handle).is_none(),
        "100ms-Delay nach 200ms muss disposed Instanz gepurgt haben"
    );
}

#[test]
fn finite_nowriter_delay_purges_after_window() {
    let rqos = DataReaderQos {
        reader_data_lifecycle: ReaderDataLifecycleQosPolicy {
            autopurge_disposed_samples_delay: QosDuration::INFINITE,
            autopurge_nowriter_samples_delay: QosDuration::from_millis(100),
        },
        ..Default::default()
    };
    let (w, r) = pair_with_rqos(rqos);

    let s = KeyedRecord { id: 3, value: 30 };
    w.write(&s).expect("write");
    pump(&w, &r);
    let _ = r.take().expect("take");

    let mut holder = PlainCdr2BeKeyHolder::new();
    s.encode_key_holder_be(&mut holder);
    let key_bytes = holder.as_bytes().to_vec();
    let max = KeyedRecord::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
    let kh = zerodds_dcps::dds_type::compute_key_hash(&key_bytes, max);
    r.__push_lifecycle(kh, key_bytes, InstanceStateKind::NotAliveNoWriters)
        .expect("push lifecycle");
    let _ = r.take().expect("take nowriter marker");

    thread::sleep(Duration::from_millis(200));
    let _ = r.take().expect("take after purge window");

    let tracker = r.instance_tracker();
    let handle = r.lookup_instance(&s);
    assert!(
        tracker.get_by_handle(handle).is_none(),
        "100ms-NoWriter-Delay nach 200ms muss Instanz gepurgt haben"
    );
}
