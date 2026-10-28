// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The one instance of a topic without a key (DDS 1.4 §2.2.1.2.2): "If no
//! key is provided, the data set associated with the Topic is restricted to
//! a single instance." The writer registers, looks up, disposes and
//! unregisters it; the reader gives it a handle, a NEW view state at the
//! first sample, and NOT_ALIVE_DISPOSED / NOT_ALIVE_NO_WRITERS markers
//! (§2.2.2.5.1.3, §2.2.2.5.1.13).

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    HANDLE_NIL, InstanceStateKind, PublisherQos, SubscriberQos, TopicQos, ViewStateKind,
    dds_type::{DecodeError, EncodeError},
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Plain {
    value: u32,
}

impl DdsType for Plain {
    const TYPE_NAME: &'static str = "test::KeylessPlain";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = false;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.value.to_be_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let head: [u8; 4] =
            bytes
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .ok_or(DecodeError::Invalid {
                    what: "truncated Plain",
                })?;
        Ok(Self {
            value: u32::from_be_bytes(head),
        })
    }
}

/// KEEP_ALL: the topic has one instance, so the default KEEP_LAST(1) of
/// §2.2.3.18 would keep only the last sample of it.
fn keep_all() -> DataReaderQos {
    DataReaderQos {
        history: zerodds_qos::HistoryQosPolicy {
            kind: zerodds_qos::HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

fn pair() -> (
    zerodds_dcps::DataWriter<Plain>,
    zerodds_dcps::DataReader<Plain>,
) {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = p
        .create_topic::<Plain>("Keyless", TopicQos::default())
        .unwrap();
    let w = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Plain>(&topic, DataWriterQos::default())
        .unwrap();
    let r = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Plain>(&topic, keep_all())
        .unwrap();
    (w, r)
}

fn pump(w: &zerodds_dcps::DataWriter<Plain>, r: &zerodds_dcps::DataReader<Plain>) {
    for b in w.__drain_pending() {
        r.__push_raw(b).unwrap();
    }
}

/// The writer's one instance can be registered, found and disposed; before,
/// `lookup_instance` answered HANDLE_NIL and `dispose` was BAD_PARAMETER.
#[test]
fn the_writer_disposes_the_one_instance_of_a_keyless_topic() {
    let (w, _) = pair();
    let sample = Plain { value: 1 };
    assert_eq!(w.lookup_instance(&sample), HANDLE_NIL, "not registered yet");
    let handle = w.register_instance(&sample).unwrap();
    assert!(!handle.is_nil());
    // Any value names the same instance.
    assert_eq!(w.lookup_instance(&Plain { value: 99 }), handle);
    assert_eq!(w.register_instance(&Plain { value: 2 }).unwrap(), handle);

    w.dispose(&sample, HANDLE_NIL).unwrap();
    assert_eq!(
        w.instance_tracker().get_by_handle(handle).unwrap().kind,
        InstanceStateKind::NotAliveDisposed
    );
}

/// A write registers the instance, as for a keyed topic, and a write after
/// the dispose makes it ALIVE again.
#[test]
fn a_write_registers_the_one_instance() {
    let (w, _) = pair();
    let sample = Plain { value: 1 };
    w.write(&sample).unwrap();
    let handle = w.lookup_instance(&sample);
    assert!(!handle.is_nil());
    w.unregister_instance(&sample, HANDLE_NIL).unwrap();
    w.write(&sample).unwrap();
    assert_eq!(
        w.instance_tracker().get_by_handle(handle).unwrap().kind,
        InstanceStateKind::Alive
    );
}

/// The reader names the one instance: a handle that is not HANDLE_NIL, the
/// view_state NEW for every sample of one call that reaches the instance for
/// the first time and NOT_NEW once it was accessed (§2.2.2.5.1.8, Figure
/// 2.11: the view_state belongs to the instance, not to a sample).
#[test]
fn the_reader_gives_the_one_instance_a_handle_and_a_view_state() {
    let (w, r) = pair();
    w.write(&Plain { value: 1 }).unwrap();
    w.write(&Plain { value: 2 }).unwrap();
    pump(&w, &r);
    let samples = r.take_with_info().unwrap();
    assert_eq!(samples.len(), 2);
    assert!(!samples[0].info.instance_handle.is_nil());
    assert_eq!(
        samples[0].info.instance_handle,
        samples[1].info.instance_handle
    );
    assert_eq!(samples[0].info.view_state, ViewStateKind::New);
    assert_eq!(samples[1].info.view_state, ViewStateKind::New);
    assert_eq!(
        r.lookup_instance(&Plain { value: 7 }),
        samples[0].info.instance_handle
    );
    w.write(&Plain { value: 3 }).unwrap();
    pump(&w, &r);
    let later = r.take_with_info().unwrap();
    assert_eq!(later.len(), 1);
    assert_eq!(later[0].info.view_state, ViewStateKind::NotNew);
}

/// A dispose of the one instance reaches the reader as an invalid-data
/// sample in NOT_ALIVE_DISPOSED (§2.2.2.5.1.3) on the instance of the data;
/// a write after it makes the instance ALIVE again with the generation
/// count raised (§2.2.2.5.1.5).
#[test]
fn a_dispose_reaches_the_reader_as_not_alive_disposed() {
    let (w, r) = pair();
    w.write(&Plain { value: 5 }).unwrap();
    pump(&w, &r);
    let data = r.take_with_info().unwrap();
    assert_eq!(data[0].info.instance_state, InstanceStateKind::Alive);

    r.__push_lifecycle([0xAB; 16], Vec::new(), InstanceStateKind::NotAliveDisposed)
        .unwrap();
    let marker = r.take_with_info().unwrap();
    assert_eq!(marker.len(), 1);
    assert!(!marker[0].info.valid_data);
    assert_eq!(
        marker[0].info.instance_state,
        InstanceStateKind::NotAliveDisposed
    );
    assert_eq!(marker[0].info.instance_handle, data[0].info.instance_handle);

    w.write(&Plain { value: 6 }).unwrap();
    pump(&w, &r);
    let again = r.take_with_info().unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].info.instance_state, InstanceStateKind::Alive);
    assert_eq!(again[0].info.disposed_generation_count, 1);
}

/// An unregister of the one instance by its last writer is
/// NOT_ALIVE_NO_WRITERS (§2.2.2.5.1.3).
#[test]
fn an_unregister_reaches_the_reader_as_not_alive_no_writers() {
    let (w, r) = pair();
    w.write(&Plain { value: 5 }).unwrap();
    pump(&w, &r);
    let _ = r.take_with_info().unwrap();
    r.__push_lifecycle([0u8; 16], Vec::new(), InstanceStateKind::NotAliveNoWriters)
        .unwrap();
    let marker = r.take_with_info().unwrap();
    assert_eq!(marker.len(), 1);
    assert_eq!(
        marker[0].info.instance_state,
        InstanceStateKind::NotAliveNoWriters
    );
}

/// DDS 1.4 §2.2.2.4.2.9: `get_key_value` fills the key fields of an instance
/// of a valid handle; the one instance of a topic without a key has no key
/// fields, so it answers with a value of the type, not with UNSUPPORTED.
#[test]
fn get_key_value_answers_for_the_one_instance_of_a_keyless_topic() {
    let (w, r) = pair();
    let handle = w.register_instance(&Plain { value: 7 }).unwrap();
    assert_eq!(w.get_key_value(handle).unwrap(), Plain { value: 7 });
    w.write(&Plain { value: 8 }).unwrap();
    pump(&w, &r);
    let samples = r.take_with_info().unwrap();
    let reader_handle = samples[0].info.instance_handle;
    assert_eq!(r.get_key_value(reader_handle).unwrap().value, 8);
    // A handle of no instance is the one error §2.2.2.4.2.9 names.
    assert!(matches!(
        w.get_key_value(zerodds_dcps::InstanceHandle::from_raw(0xDEAD)),
        Err(zerodds_dcps::DdsError::BadParameter { .. })
    ));
}
