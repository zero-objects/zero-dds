// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The PARTITION is not part of the identity of an instance (DDS 1.4
//! §2.2.3.13 compares a partition with a domain; an instance is the tuple
//! (domainId, Topic, key)): the same key written into two partitions that
//! one reader joins is one instance.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};

#[path = "common/mod.rs"]
mod common;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Rec {
    id: u32,
    value: u32,
}

impl DdsType for Rec {
    const TYPE_NAME: &'static str = "test::PartRec";
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

fn partitioned(name: &str) -> zerodds_qos::PartitionQosPolicy {
    zerodds_qos::PartitionQosPolicy {
        names: vec![name.to_string()],
    }
}

/// A reader in the partitions A and B gets the key 7 from a writer in A and
/// from a writer in B. Both samples are of one instance: one InstanceHandle,
/// and the second is not a NEW instance.
#[test]
fn one_key_in_two_partitions_is_one_instance() {
    let lease = common::lease_domain();
    let p = common::ParticipantGuard::new(
        DomainParticipantFactory::instance()
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos::default(),
                common::isolated_cfg(),
            )
            .unwrap(),
    );
    let topic = p
        .create_topic::<Rec>("PartitionInstance", TopicQos::default())
        .unwrap();
    let writer_in = |name: &str| {
        p.create_publisher(PublisherQos {
            partition: partitioned(name),
            ..PublisherQos::default()
        })
        .create_datawriter::<Rec>(&topic, DataWriterQos::default())
        .unwrap()
    };
    let in_a = writer_in("A");
    let in_b = writer_in("B");
    let reader = p
        .create_subscriber(SubscriberQos {
            partition: zerodds_qos::PartitionQosPolicy {
                names: vec!["A".into(), "B".into()],
            },
            ..SubscriberQos::default()
        })
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

    let deadline = std::time::Instant::now() + common::match_timeout();
    while reader.matched_publication_count() < 2 {
        assert!(
            std::time::Instant::now() < deadline,
            "the reader matched {} of its two writers",
            reader.matched_publication_count()
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    in_a.write(&Rec { id: 7, value: 1 }).unwrap();
    in_b.write(&Rec { id: 7, value: 2 }).unwrap();
    let mut got = Vec::new();
    while got.len() < 2 {
        assert!(
            std::time::Instant::now() < deadline,
            "{} of two samples arrived",
            got.len()
        );
        got.extend(reader.take_with_info().unwrap());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(got.len(), 2);
    assert_eq!(
        got[0].info.instance_handle, got[1].info.instance_handle,
        "one key, one instance, whatever the partition"
    );
    assert_ne!(
        got[0].info.publication_handle, got[1].info.publication_handle,
        "the writers are two"
    );
    // The second sample is not the first of a new instance.
    assert_eq!(got.iter().map(|s| s.data.value).sum::<u32>(), 3);
    let handle = got[0].info.instance_handle;
    assert_eq!(reader.get_key_value(handle).unwrap().id, 7);
}
