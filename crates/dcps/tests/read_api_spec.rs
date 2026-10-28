// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `read_w_condition` and `take_w_condition` with a ReadCondition and with the
//! reader's own conditions only, `read_next_sample`, `take_next_sample`,
//! `read_next_instance_w_condition`, `take_next_instance_w_condition`,
//! `return_loan` and `get_topicdescription` of the DataReader
//! (DDS 1.4 §2.2.2.5.3.10 to §2.2.2.5.3.20, §2.2.2.5.3.27).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipantFactory,
    DomainParticipantQos, HANDLE_NIL, InstanceHandle, PublisherQos, ReadSelection, SampleStateKind,
    SubscriberQos, TopicDescription, TopicDescriptionKind, TopicQos,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Rec {
    id: u32,
    value: u32,
}

impl DdsType for Rec {
    const TYPE_NAME: &'static str = "test::ApiRec";
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

    fn field_value(&self, path: &str) -> Option<zerodds_dcps::FilterValue> {
        match path {
            "id" => Some(zerodds_dcps::FilterValue::Int(i64::from(self.id))),
            "value" => Some(zerodds_dcps::FilterValue::Int(i64::from(self.value))),
            _ => None,
        }
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

struct Rig {
    participant: zerodds_dcps::DomainParticipant,
    topic: zerodds_dcps::Topic<Rec>,
    subscriber: zerodds_dcps::Subscriber,
    writer: zerodds_dcps::DataWriter<Rec>,
    reader: zerodds_dcps::DataReader<Rec>,
}

fn keep_all() -> DataReaderQos {
    DataReaderQos {
        history: zerodds_qos::HistoryQosPolicy {
            kind: zerodds_qos::HistoryKind::KeepAll,
            depth: 1,
        },
        ..DataReaderQos::default()
    }
}

fn rig() -> Rig {
    let participant = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let topic = participant
        .create_topic::<Rec>("ApiTopic", TopicQos::default())
        .unwrap();
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Rec>(&topic, DataWriterQos::default())
        .unwrap();
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<Rec>(&topic, keep_all())
        .unwrap();
    Rig {
        participant,
        topic,
        subscriber,
        writer,
        reader,
    }
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

/// §2.2.2.5.3.10: the parameter of `read_w_condition` is a ReadCondition,
/// which a QueryCondition is one of. A plain ReadCondition selects by its
/// three state masks only, and what is returned changes state as for `read`
/// (`take_w_condition` removes it).
#[test]
fn a_plain_read_condition_selects_by_its_state_masks() {
    let r = rig();
    feed(&r.writer, &r.reader, &[rec(1, 1), rec(2, 2)]);
    let not_read = r
        .reader
        .create_readcondition(
            sample_state_mask::NOT_READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    let read = r
        .reader
        .create_readcondition(
            sample_state_mask::READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();

    assert!(r.reader.read_w_condition(&read).unwrap().is_empty());
    let first = r.reader.read_w_condition(&not_read).unwrap();
    assert_eq!(first.len(), 2);
    assert!(
        first
            .iter()
            .all(|s| s.info.sample_state == SampleStateKind::NotRead)
    );
    // They are READ now: the NOT_READ condition has none, the READ one two.
    assert!(r.reader.read_w_condition(&not_read).unwrap().is_empty());
    let again = r.reader.read_w_condition(&read).unwrap();
    assert_eq!(again.len(), 2);
    assert!(
        again
            .iter()
            .all(|s| s.info.sample_state == SampleStateKind::Read)
    );

    // `max_samples` of the ReadCondition forms.
    assert_eq!(
        r.reader.read_w_condition_selected(&read, 1).unwrap().len(),
        1
    );
    assert!(matches!(
        r.reader.take_w_condition_selected(&not_read, -1),
        Err(DdsError::NoData)
    ));
    let taken = r.reader.take_w_condition(&read).unwrap();
    assert_eq!(taken.len(), 2);
    assert!(matches!(
        r.reader.read_selected(&ReadSelection::ANY),
        Err(DdsError::NoData)
    ));
}

/// §2.2.2.5.3.10: "The ReadCondition must be a condition created by the
/// DataReader"; any other is PRECONDITION_NOT_MET, for `read_w_condition`
/// and for `take_w_condition`, with a ReadCondition and a QueryCondition.
#[test]
fn a_condition_of_another_reader_is_precondition_not_met() {
    let r = rig();
    let other = r
        .subscriber
        .create_datareader::<Rec>(&r.topic, keep_all())
        .unwrap();
    feed(&r.writer, &r.reader, &[rec(1, 1)]);
    let foreign_read = other
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    let foreign_query = other
        .create_querycondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            "value > 0",
            Vec::new(),
        )
        .unwrap();
    let free = zerodds_dcps::ReadCondition::new(
        sample_state_mask::ANY,
        view_state_mask::ANY,
        instance_state_mask::ANY,
        |_, _, _| true,
    );
    let pnm = |e: Result<Vec<zerodds_dcps::Sample<Rec>>, DdsError>| {
        matches!(e, Err(DdsError::PreconditionNotMet { .. }))
    };
    assert!(pnm(r.reader.read_w_condition(&foreign_read)));
    assert!(pnm(r.reader.take_w_condition(&foreign_read)));
    assert!(pnm(r.reader.read_w_condition(&foreign_query)));
    assert!(pnm(r.reader.take_w_condition(&foreign_query)));
    assert!(pnm(r.reader.read_w_condition(&free)));
    assert!(pnm(r.reader.take_w_condition_selected(&free, -1)));
    assert!(pnm(r
        .reader
        .read_next_instance_w_condition(HANDLE_NIL, &foreign_read)));
    // Nothing was read by the refused calls.
    assert_eq!(
        r.reader.read_selected(&ReadSelection::ANY).unwrap().len(),
        1
    );
    // The reader's own condition works.
    let mine = r
        .reader
        .create_readcondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
        .unwrap();
    assert_eq!(r.reader.read_w_condition(&mine).unwrap().len(), 1);
}

/// §2.2.2.5.3.12 and .13: the next not-previously-accessed sample, one at a
/// time, in arrival order; NO_DATA when there is none.
#[test]
fn read_next_sample_and_take_next_sample_give_one_unaccessed_sample() {
    let r = rig();
    assert!(matches!(r.reader.read_next_sample(), Err(DdsError::NoData)));
    assert!(matches!(r.reader.take_next_sample(), Err(DdsError::NoData)));
    feed(&r.writer, &r.reader, &[rec(1, 10), rec(2, 20), rec(3, 30)]);

    let a = r.reader.read_next_sample().unwrap();
    assert_eq!(a.data.value, 10);
    let b = r.reader.read_next_sample().unwrap();
    assert_eq!(b.data.value, 20, "the first is READ and is passed over");
    // take_next_sample takes the next one not accessed and leaves the READ ones.
    let c = r.reader.take_next_sample().unwrap();
    assert_eq!(c.data.value, 30);
    assert!(matches!(r.reader.take_next_sample(), Err(DdsError::NoData)));
    assert!(matches!(r.reader.read_next_sample(), Err(DdsError::NoData)));
    let left = r.reader.read_selected(&ReadSelection::ANY).unwrap();
    let values: Vec<u32> = left.iter().map(|s| s.data.value).collect();
    assert_eq!(
        values,
        vec![10, 20],
        "the two READ samples stay in the reader"
    );
}

/// §2.2.2.5.3.18 and .19: the samples of the one instance with the smallest
/// handle above `previous` that has samples the condition accepts; the
/// iteration ends in NO_DATA.
#[test]
fn next_instance_w_condition_walks_the_instances_the_condition_accepts() {
    let r = rig();
    feed(
        &r.writer,
        &r.reader,
        &[rec(1, 5), rec(2, 20), rec(3, 30), rec(2, 6), rec(4, 7)],
    );
    let all = r.reader.read_selected(&ReadSelection::ANY).unwrap();
    let handle = |id: u32| {
        all.iter()
            .find(|s| s.data.id == id)
            .unwrap()
            .info
            .instance_handle
    };
    let mut accepted = [handle(2), handle(3)];
    accepted.sort();
    let big = r
        .reader
        .create_querycondition(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
            "value > 15",
            Vec::new(),
        )
        .unwrap();

    let first = r
        .reader
        .read_next_instance_w_condition(HANDLE_NIL, &big)
        .unwrap();
    assert_eq!(first.len(), 1, "only the sample with value > 15");
    assert_eq!(first[0].info.instance_handle, accepted[0]);
    let second = r
        .reader
        .read_next_instance_w_condition(accepted[0], &big)
        .unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].info.instance_handle, accepted[1]);
    assert!(matches!(
        r.reader.read_next_instance_w_condition(accepted[1], &big),
        Err(DdsError::NoData)
    ));
    // A previous handle that is no instance of the reader is allowed.
    assert!(
        r.reader
            .read_next_instance_w_condition(InstanceHandle::from_raw(1), &big)
            .is_ok()
    );

    // take: the accepted sample of the first instance leaves, the other
    // samples of that instance stay.
    let taken = r
        .reader
        .take_next_instance_w_condition(HANDLE_NIL, &big)
        .unwrap();
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0].info.instance_handle, accepted[0]);
    let next = r
        .reader
        .take_next_instance_w_condition(accepted[0], &big)
        .unwrap();
    assert_eq!(next[0].info.instance_handle, accepted[1]);
    assert!(matches!(
        r.reader.take_next_instance_w_condition(HANDLE_NIL, &big),
        Err(DdsError::NoData)
    ));
    let left: Vec<(u32, u32)> = r
        .reader
        .read_selected(&ReadSelection::ANY)
        .unwrap()
        .iter()
        .map(|s| (s.data.id, s.data.value))
        .collect();
    assert_eq!(left.len(), 3, "the samples below 15 stay: {left:?}");
    assert!(left.iter().all(|(_, v)| *v < 15));
}

/// §2.2.2.5.3.20: this API returns the samples owned and loans nothing, so
/// `return_loan` has no loan to give back and leaves the reader as it is;
/// after the reader is deleted it is an error as every operation is.
#[test]
fn return_loan_leaves_the_reader_as_it_is() {
    let r = rig();
    feed(&r.writer, &r.reader, &[rec(1, 1), rec(2, 2)]);
    let samples = r.reader.read_selected(&ReadSelection::ANY).unwrap();
    assert_eq!(samples.len(), 2);
    r.reader.return_loan(samples).unwrap();
    r.reader.return_loan(Vec::new()).unwrap();
    assert_eq!(
        r.reader.read_selected(&ReadSelection::ANY).unwrap().len(),
        2
    );
    r.subscriber.delete_datareader(&r.reader).unwrap();
    assert!(matches!(
        r.reader.return_loan(Vec::new()),
        Err(DdsError::AlreadyDeleted)
    ));
}

/// §2.2.2.5.3.27: `get_topicdescription` is the TopicDescription the reader
/// was created with: the Topic, or the ContentFilteredTopic of
/// `create_datareader_with_cft`, whose related Topic `topic()` still gives.
#[test]
fn get_topicdescription_is_the_description_the_reader_was_created_with() {
    let r = rig();
    let plain = r.reader.get_topicdescription().unwrap();
    assert_eq!(plain.kind(), TopicDescriptionKind::Topic);
    assert_eq!(plain.get_name(), "ApiTopic");

    let cft = r
        .participant
        .create_contentfilteredtopic("ApiCft", &r.topic, "value > 1", Vec::new())
        .unwrap();
    let filtered = r
        .subscriber
        .create_datareader_with_cft(&cft, keep_all())
        .unwrap();
    let described = filtered.get_topicdescription().unwrap();
    assert_eq!(described.kind(), TopicDescriptionKind::ContentFilteredTopic);
    assert_eq!(described.get_name(), "ApiCft");
    assert_eq!(filtered.topic().name(), "ApiTopic", "the related Topic");
    // The handle gives the typed object back.
    let again = described.content_filtered_topic::<Rec>().unwrap();
    assert_eq!(again.get_name(), "ApiCft");
    // The reader still holds the CFT: it is not deleted.
    assert!(r.participant.delete_contentfilteredtopic(&cft).is_err());
    r.subscriber.delete_datareader(&filtered).unwrap();
    r.participant.delete_contentfilteredtopic(&cft).unwrap();
}
