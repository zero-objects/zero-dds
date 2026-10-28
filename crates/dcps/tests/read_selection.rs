// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `max_samples`, the state masks of the `*_instance` operations and NO_DATA
//! (DDS 1.4 §2.2.2.5.3.8 to §2.2.2.5.3.17).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipantFactory,
    DomainParticipantQos, HANDLE_NIL, PublisherQos, ReadSelection, SampleStateKind, SubscriberQos,
    TopicQos,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Rec {
    id: u32,
    value: u32,
}

impl DdsType for Rec {
    const TYPE_NAME: &'static str = "test::SelRec";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_be_bytes());
        out.extend_from_slice(&self.value.to_be_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let id: [u8; 4] = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "id" })?;
        let value: [u8; 4] = bytes
            .get(4..8)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "value" })?;
        Ok(Self {
            id: u32::from_be_bytes(id),
            value: u32::from_be_bytes(value),
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let id: [u8; 4] = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid { what: "key" })?;
        Ok(Self {
            id: u32::from_be_bytes(id),
            value: 0,
        })
    }
}

fn pair() -> (zerodds_dcps::DataWriter<Rec>, zerodds_dcps::DataReader<Rec>) {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = p.create_topic::<Rec>("Sel", TopicQos::default()).unwrap();
    let w = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Rec>(&topic, DataWriterQos::default())
        .unwrap();
    let r = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Rec>(
            &topic,
            DataReaderQos {
                history: zerodds_qos::HistoryQosPolicy {
                    kind: zerodds_qos::HistoryKind::KeepAll,
                    depth: 1,
                },
                ..DataReaderQos::default()
            },
        )
        .unwrap();
    (w, r)
}

fn feed(w: &zerodds_dcps::DataWriter<Rec>, r: &zerodds_dcps::DataReader<Rec>, samples: &[Rec]) {
    for s in samples {
        w.write(s).unwrap();
    }
    for b in w.__drain_pending() {
        r.__push_raw(b).unwrap();
    }
}

fn rec(id: u32, value: u32) -> Rec {
    Rec { id, value }
}

/// "The size of the returned collection will be limited to the specified
/// max_samples"; what is not returned stays as it was.
#[test]
fn max_samples_limits_read_and_take() {
    let (w, r) = pair();
    feed(&w, &r, &[rec(1, 1), rec(1, 2), rec(1, 3)]);

    let two = r.read_selected(&ReadSelection::limited(2)).unwrap();
    assert_eq!(two.len(), 2);
    // Only the two returned samples became READ.
    let not_read = r
        .read_selected(&ReadSelection {
            sample_states: sample_state_mask::NOT_READ,
            ..ReadSelection::ANY
        })
        .unwrap();
    assert_eq!(not_read.len(), 1);
    assert_eq!(not_read[0].data.value, 3);

    let one = r.take_selected(&ReadSelection::limited(1)).unwrap();
    assert_eq!(one.len(), 1);
    let rest = r.take_selected(&ReadSelection::ANY).unwrap();
    assert_eq!(rest.len(), 2, "one was taken, two remain");
    assert!(matches!(
        r.take_selected(&ReadSelection::ANY),
        Err(DdsError::NoData)
    ));
}

/// "If the DataReader has no samples that meet the constraints, the return
/// value will be NO_DATA"; a `max_samples` below LENGTH_UNLIMITED is no count.
#[test]
fn no_matching_sample_is_no_data() {
    let (w, r) = pair();
    assert!(matches!(
        r.read_selected(&ReadSelection::ANY),
        Err(DdsError::NoData)
    ));
    feed(&w, &r, &[rec(1, 1)]);
    assert!(matches!(
        r.read_selected(&ReadSelection::limited(-2)),
        Err(DdsError::BadParameter { .. })
    ));
    assert!(matches!(
        r.read_selected(&ReadSelection {
            instance_states: instance_state_mask::NOT_ALIVE,
            ..ReadSelection::ANY
        }),
        Err(DdsError::NoData)
    ));
    assert!(matches!(
        r.read_selected(&ReadSelection::limited(0)),
        Err(DdsError::NoData)
    ));
    assert_eq!(r.read_selected(&ReadSelection::ANY).unwrap().len(), 1);
}

/// §2.2.2.5.3.14 and .15: read_instance and take_instance take
/// `max_samples` and the three masks.
#[test]
fn instance_reads_take_masks_and_a_limit() {
    let (w, r) = pair();
    feed(&w, &r, &[rec(1, 1), rec(1, 2), rec(2, 1)]);
    let all = r.read_selected(&ReadSelection::ANY).unwrap();
    let h1 = all
        .iter()
        .find(|s| s.data.id == 1)
        .unwrap()
        .info
        .instance_handle;
    let h2 = all
        .iter()
        .find(|s| s.data.id == 2)
        .unwrap()
        .info
        .instance_handle;

    // All three are READ now.
    assert!(matches!(
        r.read_instance_selected(
            h1,
            &ReadSelection {
                sample_states: sample_state_mask::NOT_READ,
                ..ReadSelection::ANY
            }
        ),
        Err(DdsError::NoData)
    ));
    let one = r
        .read_instance_selected(h1, &ReadSelection::limited(1))
        .unwrap();
    assert_eq!(one.len(), 1);
    assert!(one.iter().all(|s| s.info.instance_handle == h1));
    assert!(matches!(
        r.read_instance_selected(HANDLE_NIL, &ReadSelection::ANY),
        Err(DdsError::BadParameter { .. })
    ));
    // An instance nobody knows has no samples.
    assert!(matches!(
        r.take_instance_selected(
            zerodds_dcps::InstanceHandle::from_raw(0xDEAD_BEEF),
            &ReadSelection::ANY
        ),
        Err(DdsError::NoData)
    ));
    let taken = r
        .take_instance_selected(
            h2,
            &ReadSelection {
                view_states: view_state_mask::NOT_NEW,
                ..ReadSelection::ANY
            },
        )
        .unwrap();
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0].info.sample_state, SampleStateKind::Read);
}

/// §2.2.2.5.3.16: the next instance is the smallest handle above `previous`
/// that has samples in the states asked for; none is NO_DATA.
#[test]
fn next_instance_looks_at_the_states() {
    let (w, r) = pair();
    feed(&w, &r, &[rec(1, 1), rec(2, 1)]);
    let all = r.read_selected(&ReadSelection::ANY).unwrap();
    let mut handles: Vec<_> = all.iter().map(|s| s.info.instance_handle).collect();
    handles.sort();
    let (low, high) = (handles[0], handles[1]);
    // A new sample for the higher instance only.
    let high_id = all
        .iter()
        .find(|s| s.info.instance_handle == high)
        .unwrap()
        .data
        .id;
    feed(&w, &r, &[rec(high_id, 2)]);

    let not_read = ReadSelection {
        sample_states: sample_state_mask::NOT_READ,
        ..ReadSelection::ANY
    };
    // The lower instance has only READ samples, so it is passed over.
    let next = r
        .read_next_instance_selected(HANDLE_NIL, &not_read)
        .unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].info.instance_handle, high);
    assert!(matches!(
        r.read_next_instance_selected(high, &not_read),
        Err(DdsError::NoData)
    ));
    // Any state: the lower one comes first.
    let first = r
        .take_next_instance_selected(HANDLE_NIL, &ReadSelection::ANY)
        .unwrap();
    assert!(first.iter().all(|s| s.info.instance_handle == low));
}
