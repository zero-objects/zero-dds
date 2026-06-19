//! Integration tests for SampleInfo + instance lifecycle.
//!
//! Spec reference: OMG DDS-DCPS 1.4 §2.2.2.5.1 (`SampleInfo`),
//! §2.2.2.4.2.{5,7,10,13,14}, §2.2.2.5.3.{27,28}.
//!
//! We use a small keyed test fixture `KeyedRecord` whose
//! `encode`/`decode` can read from the **same** PLAIN_CDR2-BE key holder.
//! This makes `get_key_value` work exactly per spec.

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

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipantFactory,
    DomainParticipantQos, HANDLE_NIL, InstanceStateKind, PublisherQos, SampleStateKind,
    SubscriberQos, TopicQos, ViewStateKind,
    dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder},
};

extern crate alloc;
use alloc::vec::Vec;

/// Test fixture: keyed topic with u32 key + u32 value. Encode/decode
/// use **big-endian** (PLAIN_CDR2-BE) so that `get_key_value` can decode
/// directly from the stored key holder.
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
        // At least id must be present. value defaults to 0 for lifecycle
        // markers / get_key_value (key-only stream).
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
        .create_topic::<KeyedRecord>("KR", TopicQos::default())
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

/// Helper: pump all pending bytes from the writer into the reader.
fn pump(w: &zerodds_dcps::DataWriter<KeyedRecord>, r: &zerodds_dcps::DataReader<KeyedRecord>) {
    for b in w.__drain_pending() {
        r.__push_raw(b).expect("push raw");
    }
}

// =====================================================================
// Stage E: 20+ tests
// =====================================================================

#[test]
fn writer_register_instance_returns_stable_handle() {
    let (w, _) = mk_pair();
    let r = KeyedRecord { id: 42, value: 0 };
    let h1 = w.register_instance(&r).expect("register");
    let h2 = w.register_instance(&r).expect("register again");
    assert_eq!(h1, h2);
    assert!(!h1.is_nil());
}

#[test]
fn writer_lookup_instance_finds_registered_handle() {
    let (w, _) = mk_pair();
    let r = KeyedRecord { id: 7, value: 0 };
    let h = w.register_instance(&r).expect("register");
    let looked = w.lookup_instance(&r);
    assert_eq!(looked, h);
}

#[test]
fn writer_lookup_instance_returns_nil_for_unknown() {
    let (w, _) = mk_pair();
    let r = KeyedRecord { id: 99, value: 0 };
    assert_eq!(w.lookup_instance(&r), HANDLE_NIL);
}

#[test]
fn writer_get_key_value_extracts_only_key() {
    let (w, _) = mk_pair();
    let r = KeyedRecord {
        id: 0xCAFE_BABE,
        value: 0xDEAD_BEEF,
    };
    let h = w.register_instance(&r).expect("register");
    let key_only = w.get_key_value(h).expect("get_key_value");
    assert_eq!(key_only.id, r.id);
    // Value was **not** stored → default 0.
    assert_eq!(key_only.value, 0);
}

#[test]
fn writer_get_key_value_unknown_handle_errors() {
    let (w, _) = mk_pair();
    let bad = zerodds_dcps::InstanceHandle::from_raw(0xDEAD_BEEF);
    assert!(matches!(
        w.get_key_value(bad),
        Err(DdsError::BadParameter { .. })
    ));
}

#[test]
fn writer_dispose_transitions_instance_to_disposed() {
    let (w, _) = mk_pair();
    let r = KeyedRecord { id: 1, value: 0 };
    let h = w.register_instance(&r).expect("register");
    assert_eq!(
        w.instance_tracker().get_by_handle(h).unwrap().kind,
        InstanceStateKind::Alive
    );
    w.dispose(&r, h).expect("dispose");
    assert_eq!(
        w.instance_tracker().get_by_handle(h).unwrap().kind,
        InstanceStateKind::NotAliveDisposed
    );
}

#[test]
fn writer_unregister_drops_to_no_writers_when_count_zero() {
    // Spec §2.2.3.21: with autodispose_unregistered_instances=false,
    // unregister should fall ONLY to NotAliveNoWriters (without dispose).
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<KeyedRecord>("KR", TopicQos::default())
        .expect("topic");
    let pub_ = p.create_publisher(PublisherQos::default());
    let qos = DataWriterQos {
        writer_data_lifecycle: zerodds_qos::WriterDataLifecycleQosPolicy {
            autodispose_unregistered_instances: false,
        },
        ..Default::default()
    };
    let w = pub_
        .create_datawriter::<KeyedRecord>(&topic, qos)
        .expect("writer");

    let r = KeyedRecord { id: 2, value: 0 };
    let h = w.register_instance(&r).expect("register");
    w.unregister_instance(&r, h).expect("unregister");
    assert_eq!(
        w.instance_tracker().get_by_handle(h).unwrap().kind,
        InstanceStateKind::NotAliveNoWriters
    );
}

#[test]
fn writer_dispose_unknown_handle_errors() {
    let (w, _) = mk_pair();
    let r = KeyedRecord { id: 3, value: 0 };
    let bad = zerodds_dcps::InstanceHandle::from_raw(0xDEAD);
    let err = w.dispose(&r, bad).unwrap_err();
    assert!(matches!(err, DdsError::BadParameter { .. }));
}

#[test]
fn writer_dispose_handle_nil_resolves_via_lookup() {
    let (w, _) = mk_pair();
    let r = KeyedRecord { id: 4, value: 0 };
    let _ = w.register_instance(&r).expect("register");
    // HANDLE_NIL → resolved via lookup_instance.
    w.dispose(&r, HANDLE_NIL).expect("dispose with nil");
}

#[test]
fn writer_dispose_handle_nil_unregistered_errors() {
    let (w, _) = mk_pair();
    let r = KeyedRecord { id: 5, value: 0 };
    let err = w.dispose(&r, HANDLE_NIL).unwrap_err();
    assert!(matches!(err, DdsError::BadParameter { .. }));
}

#[test]
fn writer_dispose_handle_mismatch_errors() {
    let (w, _) = mk_pair();
    let r1 = KeyedRecord { id: 10, value: 0 };
    let r2 = KeyedRecord { id: 20, value: 0 };
    let h1 = w.register_instance(&r1).expect("r1");
    let _h2 = w.register_instance(&r2).expect("r2");
    // h1 belongs to r1 — dispose(r2, h1) must fail.
    let err = w.dispose(&r2, h1).unwrap_err();
    assert!(matches!(err, DdsError::BadParameter { .. }));
}

#[test]
fn writer_register_then_unregister_then_re_register_bumps_generation() {
    // Spec §2.2.3.21: with autodispose_unregistered_instances=false,
    // unregister leads ONLY to NotAliveNoWriters; re-register bumps
    // no_writers_generation_count.
    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<KeyedRecord>("KR", TopicQos::default())
        .expect("topic");
    let pub_ = p.create_publisher(PublisherQos::default());
    let qos = DataWriterQos {
        writer_data_lifecycle: zerodds_qos::WriterDataLifecycleQosPolicy {
            autodispose_unregistered_instances: false,
        },
        ..Default::default()
    };
    let w = pub_
        .create_datawriter::<KeyedRecord>(&topic, qos)
        .expect("writer");

    let r = KeyedRecord { id: 11, value: 0 };
    let h = w.register_instance(&r).expect("register");
    w.unregister_instance(&r, h).expect("unregister");
    let _ = w.register_instance(&r).expect("re-register");
    let s = w.instance_tracker().get_by_handle(h).unwrap();
    assert_eq!(s.kind, InstanceStateKind::Alive);
    assert_eq!(s.no_writers_generation_count, 1);
}

#[test]
fn reader_lookup_instance_after_take_with_info() {
    let (w, r) = mk_pair();
    let s = KeyedRecord { id: 7, value: 100 };
    w.write(&s).unwrap();
    pump(&w, &r);
    let samples = r.take_with_info().expect("take");
    assert_eq!(samples.len(), 1);
    let h = samples[0].info.instance_handle;
    assert!(!h.is_nil());
    // lookup after receipt finds the instance.
    assert_eq!(r.lookup_instance(&s), h);
}

#[test]
fn reader_take_with_info_first_sample_is_new_and_not_read() {
    let (w, r) = mk_pair();
    w.write(&KeyedRecord { id: 1, value: 1 }).unwrap();
    pump(&w, &r);
    let samples = r.take_with_info().expect("take");
    assert_eq!(samples.len(), 1);
    let info = &samples[0].info;
    assert_eq!(info.view_state, ViewStateKind::New);
    // take() consumes → sample_state does not matter for take,
    // but must be NOT_READ (it was never read before).
    assert_eq!(info.sample_state, SampleStateKind::NotRead);
    assert_eq!(info.instance_state, InstanceStateKind::Alive);
    assert!(info.valid_data);
}

#[test]
fn reader_view_state_transitions_to_not_new_on_second_sample() {
    let (w, r) = mk_pair();
    w.write(&KeyedRecord { id: 2, value: 1 }).unwrap();
    pump(&w, &r);
    let _ = r.take_with_info().expect("first take");
    // Second write of the same instance → view_state == NOT_NEW.
    w.write(&KeyedRecord { id: 2, value: 2 }).unwrap();
    pump(&w, &r);
    let samples = r.take_with_info().expect("second take");
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].info.view_state, ViewStateKind::NotNew);
}

#[test]
fn reader_read_marks_sample_state_read() {
    let (w, r) = mk_pair();
    w.write(&KeyedRecord { id: 3, value: 1 }).unwrap();
    pump(&w, &r);
    let first = r.read_with_info().expect("first read");
    assert_eq!(first[0].info.sample_state, SampleStateKind::NotRead);
    // Second read of the same sample → sample_state = READ.
    let second = r.read_with_info().expect("second read");
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].info.sample_state, SampleStateKind::Read);
}

#[test]
fn reader_read_instance_filters_to_one_handle() {
    let (w, r) = mk_pair();
    w.write(&KeyedRecord { id: 1, value: 10 }).unwrap();
    w.write(&KeyedRecord { id: 2, value: 20 }).unwrap();
    pump(&w, &r);
    // First let everything decode.
    let all = r.read_with_info().expect("read");
    assert_eq!(all.len(), 2);
    let h1 = all
        .iter()
        .find(|s| s.data.id == 1)
        .map(|s| s.info.instance_handle)
        .unwrap();
    let h2 = all
        .iter()
        .find(|s| s.data.id == 2)
        .map(|s| s.info.instance_handle)
        .unwrap();
    let only_h1 = r.read_instance(h1).expect("read_instance");
    assert_eq!(only_h1.len(), 1);
    assert_eq!(only_h1[0].data.id, 1);
    let only_h2 = r.take_instance(h2).expect("take_instance");
    assert_eq!(only_h2.len(), 1);
    assert_eq!(only_h2[0].data.id, 2);
}

#[test]
fn reader_take_instance_with_handle_nil_errors() {
    let (_, r) = mk_pair();
    let err = r.take_instance(HANDLE_NIL).unwrap_err();
    assert!(matches!(err, DdsError::BadParameter { .. }));
}

#[test]
fn reader_read_instance_with_handle_nil_errors() {
    let (_, r) = mk_pair();
    let err = r.read_instance(HANDLE_NIL).unwrap_err();
    assert!(matches!(err, DdsError::BadParameter { .. }));
}

#[test]
fn reader_read_next_instance_iterates_all() {
    let (w, r) = mk_pair();
    w.write(&KeyedRecord { id: 1, value: 1 }).unwrap();
    w.write(&KeyedRecord { id: 2, value: 2 }).unwrap();
    w.write(&KeyedRecord { id: 3, value: 3 }).unwrap();
    pump(&w, &r);
    // Initialize the cache.
    let _ = r.read_with_info().expect("ingest");
    // Now iterate.
    let mut prev = HANDLE_NIL;
    let mut seen = Vec::<u32>::new();
    loop {
        let next = r.read_next_instance(prev).expect("read_next");
        if next.is_empty() {
            break;
        }
        seen.push(next[0].data.id);
        prev = next[0].info.instance_handle;
    }
    seen.sort_unstable();
    assert_eq!(seen, alloc::vec![1, 2, 3]);
}

#[test]
fn reader_take_next_instance_consumes() {
    let (w, r) = mk_pair();
    w.write(&KeyedRecord { id: 1, value: 1 }).unwrap();
    w.write(&KeyedRecord { id: 2, value: 2 }).unwrap();
    pump(&w, &r);
    let _ = r.read_with_info().expect("ingest");
    let first = r.take_next_instance(HANDLE_NIL).expect("take_next");
    assert_eq!(first.len(), 1);
    let prev = first[0].info.instance_handle;
    let second = r.take_next_instance(prev).expect("take_next 2");
    assert_eq!(second.len(), 1);
    assert_ne!(second[0].info.instance_handle, prev);
    let third = r
        .take_next_instance(second[0].info.instance_handle)
        .expect("take_next 3");
    assert!(third.is_empty());
}

#[test]
fn reader_get_key_value_returns_keyonly_sample() {
    let (w, r) = mk_pair();
    w.write(&KeyedRecord {
        id: 0xABCD,
        value: 0xFFFF,
    })
    .unwrap();
    pump(&w, &r);
    let s = r.take_with_info().expect("take");
    let h = s[0].info.instance_handle;
    let key_only = r.get_key_value(h).expect("get_key_value");
    assert_eq!(key_only.id, 0xABCD);
    assert_eq!(key_only.value, 0); // value not in the key holder
}

#[test]
fn reader_lifecycle_dispose_marker_yields_invalid_data_sample() {
    let (_w, r) = mk_pair();
    // Direct push of a dispose marker (simulates what the runtime does
    // in phase B as soon as a writer sends a dispose).
    let mut holder = PlainCdr2BeKeyHolder::new();
    let key_record = KeyedRecord { id: 77, value: 0 };
    key_record.encode_key_holder_be(&mut holder);
    let kh = zerodds_dcps::dds_type::compute_key_hash(holder.as_bytes(), 4);
    r.__push_lifecycle(
        kh,
        holder.as_bytes().to_vec(),
        InstanceStateKind::NotAliveDisposed,
    )
    .expect("push lifecycle");
    let s = r.take_with_info().expect("take");
    assert_eq!(s.len(), 1);
    assert!(!s[0].info.valid_data);
    assert_eq!(
        s[0].info.instance_state,
        InstanceStateKind::NotAliveDisposed
    );
}

#[test]
fn reader_take_filtered_with_state_mask_filters() {
    let (w, r) = mk_pair();
    w.write(&KeyedRecord { id: 1, value: 1 }).unwrap();
    pump(&w, &r);
    // Filter "READ only" → nothing.
    let none = r
        .take_filtered(
            zerodds_dcps::sample_state_mask::READ,
            zerodds_dcps::view_state_mask::ANY,
            zerodds_dcps::instance_state_mask::ANY,
        )
        .expect("take_filtered");
    assert!(none.is_empty());
    // NOT_READ → the sample.
    let one = r
        .take_filtered(
            zerodds_dcps::sample_state_mask::NOT_READ,
            zerodds_dcps::view_state_mask::ANY,
            zerodds_dcps::instance_state_mask::ANY,
        )
        .expect("take_filtered");
    assert_eq!(one.len(), 1);
}

#[test]
fn reader_publication_handle_field_is_handle_nil_offline() {
    let (w, r) = mk_pair();
    w.write(&KeyedRecord { id: 8, value: 0 }).unwrap();
    pump(&w, &r);
    let s = r.take_with_info().expect("take");
    // In offline mode no publication_handle is propagated.
    assert_eq!(s[0].info.publication_handle, HANDLE_NIL);
}

#[test]
fn writer_publication_handle_is_non_nil() {
    let (w, _) = mk_pair();
    assert!(!w.publication_handle().is_nil());
}

#[test]
fn writer_register_with_timestamp_records_timestamp() {
    let (w, _) = mk_pair();
    let ts = zerodds_dcps::Time::new(123, 456);
    let r = KeyedRecord { id: 88, value: 0 };
    let h = w.register_instance_w_timestamp(&r, ts).expect("register");
    let s = w.instance_tracker().get_by_handle(h).unwrap();
    assert_eq!(s.last_sample_timestamp, Some(ts));
}

#[test]
fn write_w_timestamp_auto_registers_unknown_instance() {
    let (w, _) = mk_pair();
    let s = KeyedRecord { id: 99, value: 1 };
    let ts = zerodds_dcps::Time::new(500, 0);
    w.write_w_timestamp(&s, ts).expect("write");
    let h = w.lookup_instance(&s);
    assert!(!h.is_nil());
    assert_eq!(
        w.instance_tracker()
            .get_by_handle(h)
            .unwrap()
            .last_sample_timestamp,
        Some(ts)
    );
}
