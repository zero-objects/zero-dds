//! A DataReader's instances and the writers that write them: an unregister
//! of one of several writers, and the ownership of an EXCLUSIVE instance
//! under dispose and unregister (DDS 1.4 §2.2.2.5.1.3, §2.2.3.23.3).
//!
//! Offline: the samples go into the reader's inbox as the runtime hands
//! them over, each with the GUID of its writer.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::qos::{OwnershipKind, OwnershipQosPolicy};
use zerodds_dcps::runtime::UserSample;
use zerodds_dcps::sample_info::InstanceStateKind;
use zerodds_dcps::{
    DataReader, DataReaderQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    SubscriberQos, TopicQos,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Rec {
    id: u32,
    value: u32,
}

impl DdsType for Rec {
    const TYPE_NAME: &'static str = "test::Rec";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.value.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let word = |at: usize| -> Result<u32, DecodeError> {
            let b: [u8; 4] = bytes
                .get(at..at + 4)
                .and_then(|b| b.try_into().ok())
                .ok_or(DecodeError::Invalid {
                    what: "truncated Rec",
                })?;
            Ok(u32::from_le_bytes(b))
        };
        Ok(Self {
            id: word(0)?,
            value: word(4)?,
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let head: [u8; 4] =
            bytes
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .ok_or(DecodeError::Invalid {
                    what: "truncated Rec key holder",
                })?;
        Ok(Self {
            id: u32::from_be_bytes(head),
            value: 0,
        })
    }
}

const W1: [u8; 16] = [1; 16];
const W2: [u8; 16] = [2; 16];

fn reader(topic: &str, exclusive: bool) -> DataReader<Rec> {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p
        .create_topic::<Rec>(topic, TopicQos::default())
        .expect("topic");
    let mut qos = DataReaderQos::default();
    if exclusive {
        qos.ownership = OwnershipQosPolicy {
            kind: OwnershipKind::Exclusive,
        };
    }
    p.create_subscriber(SubscriberQos::default())
        .create_datareader::<Rec>(&t, qos)
        .expect("reader")
}

fn write(r: &DataReader<Rec>, writer: [u8; 16], strength: i32, value: u32) {
    let mut bytes = Vec::new();
    Rec { id: 7, value }.encode(&mut bytes).unwrap();
    r.__push_raw_with_writer(bytes, writer, strength).unwrap();
}

fn marker(r: &DataReader<Rec>, writer: [u8; 16], kind: zerodds_rtps::history_cache::ChangeKind) {
    let mut holder = PlainCdr2BeKeyHolder::new();
    Rec { id: 7, value: 0 }.encode_key_holder_be(&mut holder);
    let key = holder.as_bytes().to_vec();
    let hash = zerodds_dcps::dds_type::compute_key_hash(&key, 4);
    r.__inbox_handle()
        .lock()
        .unwrap()
        .push(UserSample::Lifecycle {
            key_hash: hash,
            kind,
            key_holder: Some(key),
            source_timestamp: None,
            group: None,
            writer_guid: writer,
            coherent_set_sn: None,
            source_sequence_number: -1,
        });
}

use zerodds_rtps::history_cache::ChangeKind;

/// §2.2.2.5.1.3: NOT_ALIVE_NO_WRITERS once "there are no live DataWriter
/// entities writing that instance". One of two writers unregisters: the
/// instance stays ALIVE and no invalid sample reports a change that did not
/// happen; the second unregister ends it.
#[test]
fn an_unregister_of_one_of_two_writers_leaves_the_instance_alive() {
    let r = reader("TwoWriters", false);
    write(&r, W1, 0, 1);
    assert_eq!(r.take_with_info().unwrap().len(), 1);
    write(&r, W2, 0, 2);
    assert_eq!(r.take_with_info().unwrap().len(), 1);
    marker(&r, W1, ChangeKind::NotAliveUnregistered);
    assert!(
        r.take_with_info().unwrap().is_empty(),
        "W2 still writes the instance"
    );
    marker(&r, W2, ChangeKind::NotAliveUnregistered);
    let last = r.take_with_info().unwrap();
    assert_eq!(last.len(), 1);
    assert!(!last[0].info.valid_data);
    assert_eq!(
        last[0].info.instance_state,
        InstanceStateKind::NotAliveNoWriters
    );
}

/// §2.2.2.5.1.3: under EXCLUSIVE the instance "becomes NOT_ALIVE_DISPOSED
/// only if the DataWriter that 'owns' the instance explicitly disposes
/// it". The weaker writer's dispose is not taken; the owner's is.
#[test]
fn only_the_owner_disposes_an_exclusive_instance() {
    let r = reader("OwnedDispose", true);
    write(&r, W1, 10, 1);
    write(&r, W2, 5, 2);
    let first = r.take_with_info().unwrap();
    assert_eq!(first.len(), 1, "W2 does not own the instance");
    marker(&r, W2, ChangeKind::NotAliveDisposed);
    assert!(
        r.take_with_info().unwrap().is_empty(),
        "W2 cannot dispose it"
    );
    marker(&r, W1, ChangeKind::NotAliveDisposed);
    let disposed = r.take_with_info().unwrap();
    assert_eq!(disposed.len(), 1);
    assert_eq!(
        disposed[0].info.instance_state,
        InstanceStateKind::NotAliveDisposed
    );
    // §2.2.3.23.3: the readers "not see the values being written by the
    // weaker writer (even after the stronger one has disposed the
    // instance)".
    write(&r, W2, 5, 4);
    assert!(r.take_with_info().unwrap().is_empty(), "W1 still owns it");
}

/// §2.2.3.23.3: "if the current owner of an instance unregisters it, then
/// it will relinquish ownership". The weaker writer writes it next.
#[test]
fn an_owner_that_unregisters_gives_up_the_instance() {
    let r = reader("OwnedUnregister", true);
    write(&r, W1, 10, 1);
    write(&r, W2, 5, 2);
    assert_eq!(r.take_with_info().unwrap().len(), 1);
    marker(&r, W1, ChangeKind::NotAliveUnregistered);
    let _ = r.take_with_info().unwrap();
    write(&r, W2, 5, 3);
    let next = r.take_with_info().unwrap();
    assert_eq!(next.len(), 1, "W2 owns the instance now: {next:?}");
    assert_eq!(next[0].data.value, 3);
}

/// DDS 1.4 §2.2.2.5.1.3: "The instance_state available in the SampleInfo is
/// a snapshot of the instance_state of the instance at the time the
/// collection was obtained … the same for all samples in the returned
/// collection that refer to the same instance." A sample that arrived
/// before its instance was disposed is read as NOT_ALIVE_DISPOSED, and an
/// ALIVE-only mask no longer selects it.
#[test]
fn the_instance_state_of_a_sample_is_that_of_its_instance_when_read() {
    use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
    let r = reader("SnapshotState", false);
    write(&r, W1, 0, 1);
    marker(&r, W1, ChangeKind::NotAliveDisposed);
    let alive_only = r
        .read_filtered(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ALIVE,
        )
        .unwrap();
    assert!(alive_only.is_empty(), "{alive_only:?}");
    let all = r.take_with_info().unwrap();
    assert_eq!(all.len(), 2);
    for s in &all {
        assert_eq!(s.info.instance_state, InstanceStateKind::NotAliveDisposed);
    }
}

/// §2.2.2.5.1.8: the view_state is "a snapshot of the view_state of the
/// instance relative to the DataReader … at the time the collection was
/// obtained". The first read sees NEW, the second NOT_NEW.
#[test]
fn the_view_state_of_a_sample_is_that_of_its_instance_when_read() {
    use zerodds_dcps::sample_info::ViewStateKind;
    let r = reader("SnapshotView", false);
    write(&r, W1, 0, 1);
    let first = r.read_with_info().unwrap();
    assert_eq!(first[0].info.view_state, ViewStateKind::New);
    let second = r.read_with_info().unwrap();
    assert_eq!(second[0].info.view_state, ViewStateKind::NotNew);
}

/// DDS 1.4 §2.2.2.5.5: `publication_handle` "identifies locally the
/// DataWriter that modified the instance", and §2.2.2.4.2.11
/// `dispose_w_timestamp` gives the dispose a source timestamp. A dispose
/// marker's SampleInfo names its writer, as the data sample before it does,
/// and has the writer's source timestamp. It had HANDLE_NIL and the
/// reader's clock.
#[test]
fn a_dispose_marker_names_its_writer_and_its_source_time() {
    let r = reader("MarkerSource", false);
    write(&r, W1, 0, 1);
    let data = r.take_with_info().unwrap();
    assert_eq!(data.len(), 1);
    let publication = data[0].info.publication_handle;
    assert!(!publication.is_nil());

    let mut holder = PlainCdr2BeKeyHolder::new();
    Rec { id: 7, value: 0 }.encode_key_holder_be(&mut holder);
    let key = holder.as_bytes().to_vec();
    let hash = zerodds_dcps::dds_type::compute_key_hash(&key, 4);
    let source = zerodds_rtps::header_extension::HeTimestamp {
        seconds: 1_700_000_000,
        fraction: 0,
    };
    r.__inbox_handle()
        .lock()
        .unwrap()
        .push(UserSample::Lifecycle {
            key_hash: hash,
            kind: ChangeKind::NotAliveDisposed,
            key_holder: Some(key),
            source_timestamp: Some(source),
            group: None,
            writer_guid: W1,
            coherent_set_sn: None,
            source_sequence_number: -1,
        });
    let disposed = r.take_with_info().unwrap();
    assert_eq!(disposed.len(), 1);
    assert!(!disposed[0].info.valid_data);
    assert_eq!(disposed[0].info.publication_handle, publication);
    assert_eq!(
        disposed[0].info.source_timestamp,
        zerodds_dcps::time::he_timestamp_to_time(source)
    );
}

/// The unregister half of `a_dispose_marker_names_its_writer_and_its_source_time`
/// (§2.2.2.4.2.8 `unregister_instance_w_timestamp`): the marker of the last
/// writer leaving names that writer and carries its source timestamp.
#[test]
fn an_unregister_marker_names_its_writer_and_its_source_time() {
    let r = reader("UnregisterMarkerSource", false);
    write(&r, W1, 0, 1);
    let data = r.take_with_info().unwrap();
    assert_eq!(data.len(), 1);
    let publication = data[0].info.publication_handle;
    assert!(!publication.is_nil());

    let mut holder = PlainCdr2BeKeyHolder::new();
    Rec { id: 7, value: 0 }.encode_key_holder_be(&mut holder);
    let key = holder.as_bytes().to_vec();
    let hash = zerodds_dcps::dds_type::compute_key_hash(&key, 4);
    let source = zerodds_rtps::header_extension::HeTimestamp {
        seconds: 1_700_000_100,
        fraction: 0,
    };
    r.__inbox_handle()
        .lock()
        .unwrap()
        .push(UserSample::Lifecycle {
            key_hash: hash,
            kind: ChangeKind::NotAliveUnregistered,
            key_holder: Some(key),
            source_timestamp: Some(source),
            group: None,
            writer_guid: W1,
            coherent_set_sn: None,
            source_sequence_number: -1,
        });
    let unregistered = r.take_with_info().unwrap();
    assert_eq!(unregistered.len(), 1);
    assert!(!unregistered[0].info.valid_data);
    assert_eq!(unregistered[0].info.publication_handle, publication);
    assert_eq!(
        unregistered[0].info.source_timestamp,
        zerodds_dcps::time::he_timestamp_to_time(source)
    );
}

/// DDS 1.4 §2.2.3.9.2 (d): ownership changes when "a deadline with which
/// the DataWriter that owns the instance has committed is missed". The
/// owner stops; the weaker writer's sample within the deadline is not
/// taken, the one after it is. The owner kept the instance, and the weaker
/// writer stayed shut out.
#[test]
fn an_owner_that_missed_its_deadline_gives_the_instance_up() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p
        .create_topic::<Rec>("OwnerDeadline", TopicQos::default())
        .expect("topic");
    let qos = DataReaderQos {
        ownership: OwnershipQosPolicy {
            kind: OwnershipKind::Exclusive,
        },
        deadline: zerodds_qos::DeadlineQosPolicy {
            period: zerodds_qos::Duration::from_millis(200),
        },
        ..DataReaderQos::default()
    };
    let r = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Rec>(&t, qos)
        .expect("reader");
    write(&r, W1, 10, 1);
    assert_eq!(r.take_with_info().unwrap().len(), 1);
    write(&r, W2, 5, 2);
    assert!(
        r.take_with_info().unwrap().is_empty(),
        "the owner is within its deadline"
    );
    std::thread::sleep(std::time::Duration::from_millis(300));
    write(&r, W2, 5, 3);
    let taken = r.take_with_info().unwrap();
    assert_eq!(taken.len(), 1, "the owner missed its deadline");
    assert_eq!(taken[0].data.value, 3);
}

/// DDS 1.4 §2.2.3.9.2 (b): "a change in the strength value of the
/// DataWriter that owns the instance". The owner lowers its strength: its
/// sample is taken, and a writer stronger than the new strength takes the
/// instance over. The owner's own sample was refused.
#[test]
fn an_owner_that_lowers_its_strength_keeps_the_instance_until_a_stronger_one() {
    let r = reader("OwnerStrength", true);
    write(&r, W1, 10, 1);
    assert_eq!(r.take_with_info().unwrap().len(), 1);
    write(&r, W1, 3, 2);
    let own = r.take_with_info().unwrap();
    assert_eq!(own.len(), 1, "the owner's sample at its new strength");
    assert_eq!(own[0].data.value, 2);
    write(&r, W2, 5, 3);
    let over = r.take_with_info().unwrap();
    assert_eq!(over.len(), 1, "the stronger writer takes the instance");
    assert_eq!(over[0].data.value, 3);
}
