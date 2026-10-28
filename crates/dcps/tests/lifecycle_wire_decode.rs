//! WP QoS-Wiring T8b — reader-side wire-lifecycle decode.
//!
//! Spec DDS 1.4 §8.2.1.2 + §9.6.3.9: ReliableReader classifies
//! incoming DATA submessages with `key_flag=true` and PID_STATUS_INFO
//! as lifecycle markers. delivered_to_user_sample converts that into
//! a `UserSample::Lifecycle` variant, which the DCPS DataReader layer
//! feeds into the tracker via __push_lifecycle.

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
    /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
    const XCDR1_MATCHES_XCDR2: bool = true;
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
    // Direct test of the __push_lifecycle path, called by the live
    // channel on a wire-lifecycle sample.
    let (w, r) = mk_pair();
    let s = KeyedRecord { id: 7, value: 42 };
    w.write(&s).expect("write");
    for b in w.__drain_pending() {
        r.__push_raw(b).expect("push raw alive");
    }
    let _ = r.take_with_info().expect("take initial");

    // Push a lifecycle marker with the disposed bit.
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
        "Reader must see disposed marker with valid_data=false, got {samples:?}"
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
        "Reader must see unregister marker as NoWriters, got {samples:?}"
    );
}

/// DDS 1.4 §2.2.2.5.1 identifies an instance by its key. A dispose that
/// carries its `serializedKey` disposes the instance of that key even when
/// its `PID_KEY_HASH` is not the one DDSI-RTPS 2.5 §9.6.4.8 gives: zzdds
/// 0.2.0 sends the key's octets where a key of more than sixteen octets
/// takes the MD5. The reader keys a data sample by the hash it computes
/// from the sample; a dispose keyed by the writer's hash instead landed on
/// an instance of its own, without a key holder, and every `take` after it
/// failed.
#[test]
fn a_dispose_with_a_foreign_key_hash_disposes_the_instance_of_its_key() {
    let (w, r) = mk_pair();
    let s = KeyedRecord { id: 5, value: 9 };
    w.write(&s).expect("write");
    for b in w.__drain_pending() {
        r.__push_raw(b).expect("push raw alive");
    }
    let alive = r.take_with_info().expect("take alive");
    assert_eq!(alive.len(), 1);
    let handle = alive[0].info.instance_handle;

    let mut holder = PlainCdr2BeKeyHolder::new();
    s.encode_key_holder_be(&mut holder);
    let foreign_hash = [0x42u8; 16];
    let max = KeyedRecord::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
    assert_ne!(
        foreign_hash,
        zerodds_dcps::dds_type::compute_key_hash(holder.as_bytes(), max)
    );
    r.__inbox_handle()
        .lock()
        .unwrap()
        .push(zerodds_dcps::runtime::UserSample::Lifecycle {
            key_hash: foreign_hash,
            kind: zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed,
            key_holder: Some(holder.as_bytes().to_vec()),
            source_timestamp: None,
            group: None,
            writer_guid: [0u8; 16],
            coherent_set_sn: None,
            source_sequence_number: -1,
        });

    let samples = r.take_with_info().expect("take after dispose");
    assert_eq!(samples.len(), 1, "{samples:?}");
    assert!(!samples[0].info.valid_data);
    assert_eq!(
        samples[0].info.instance_state,
        InstanceStateKind::NotAliveDisposed
    );
    assert_eq!(samples[0].info.instance_handle, handle);
    assert_eq!(samples[0].data.id, 5);
}

/// DDS 1.4 §2.2.3.22: the last sample of an instance shows
/// NOT_ALIVE_NO_WRITERS or NOT_ALIVE_DISPOSED "depending on whether the
/// last writer that had ownership of the instance disposed it or not", and
/// §2.2.2.5.1.3 brings an instance back to ALIVE only when a writer writes
/// it. A writer that disposes and then is deleted unregisters the disposed
/// instance (§2.2.2.4.1.6): the instance stays NOT_ALIVE_DISPOSED, and no
/// disposed generation is counted. The marker went through ALIVE before, and
/// the reader reported NOT_ALIVE_NO_WRITERS (InterCom 4.3.1 as the
/// publisher, OMG dds-rtps `Test_FinalInstanceState_1`).
#[test]
fn an_unregister_after_a_dispose_leaves_the_instance_disposed() {
    let (w, r) = mk_pair();
    let s = KeyedRecord { id: 3, value: 1 };
    w.write(&s).expect("write");
    for b in w.__drain_pending() {
        r.__push_raw(b).expect("push raw alive");
    }
    let _ = r.take_with_info().expect("take alive");

    let mut holder = PlainCdr2BeKeyHolder::new();
    s.encode_key_holder_be(&mut holder);
    let key_bytes = holder.as_bytes().to_vec();
    let max = KeyedRecord::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
    let kh = zerodds_dcps::dds_type::compute_key_hash(&key_bytes, max);
    r.__push_lifecycle(kh, key_bytes.clone(), InstanceStateKind::NotAliveDisposed)
        .expect("dispose");
    r.__push_lifecycle(kh, key_bytes, InstanceStateKind::NotAliveNoWriters)
        .expect("unregister");

    let samples = r
        .take_with_info()
        .expect("take after dispose and unregister");
    assert!(!samples.is_empty());
    for s in &samples {
        assert!(!s.info.valid_data);
        assert_eq!(
            s.info.instance_state,
            InstanceStateKind::NotAliveDisposed,
            "{samples:?}"
        );
        assert_eq!(s.info.disposed_generation_count, 0, "{samples:?}");
    }
}
