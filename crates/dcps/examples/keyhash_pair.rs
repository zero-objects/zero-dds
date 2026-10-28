// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Writes `kh::Pair {a = 1, b = 2}` and reads every `kh::Pair` on topic
//! `KhPair`, printing the instance each sample lands in.
//!
//! The counterpart of `tools/keyhash-probe/rti-live/pair_live.c`. Run both
//! on one domain: a reader that puts the two writers' samples of the same
//! key in one instance identifies the key the same way, whatever KeyHash
//! each writer sent. The key holder is `@key short a; @key long long b;`,
//! where PLAIN_CDR2 (XTypes 1.3 §7.6.8 step 4) aligns `b` to 4 and XCDR1
//! to 8.
//!
//! ```text
//! keyhash_pair <domain> <seconds> <v-base> <xcdr: 1|2>
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::time::Duration;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Pair {
    a: i16,
    b: i64,
    v: i32,
}

fn field<const N: usize>(bytes: &[u8], at: usize) -> Result<[u8; N], DecodeError> {
    bytes
        .get(at..at + N)
        .and_then(|b| b.try_into().ok())
        .ok_or(DecodeError::Invalid {
            what: "truncated kh::Pair",
        })
}

impl DdsType for Pair {
    const TYPE_NAME: &'static str = "kh::Pair";
    const HAS_KEY: bool = true;
    // a (2), pad (2), b (8) under PLAIN_CDR2.
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(12);

    /// XCDR2, `@final`: `b` at 4 (maximum alignment 4), `v` at 12.
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.a.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&self.b.to_le_bytes());
        out.extend_from_slice(&self.v.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self {
            a: i16::from_le_bytes(field(bytes, 0)?),
            b: i64::from_le_bytes(field(bytes, 4)?),
            v: i32::from_le_bytes(field(bytes, 12)?),
        })
    }

    /// XCDR1: `b` at 8, `v` at 16.
    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.a.to_le_bytes());
        out.extend_from_slice(&[0; 6]);
        out.extend_from_slice(&self.b.to_le_bytes());
        out.extend_from_slice(&self.v.to_le_bytes());
        Ok(())
    }

    fn decode_xcdr1(bytes: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self {
            a: i16::from_le_bytes(field(bytes, 0)?),
            b: i64::from_le_bytes(field(bytes, 8)?),
            v: i32::from_le_bytes(field(bytes, 16)?),
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_i16(self.a);
        holder.write_i64(self.b);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self {
            a: i16::from_be_bytes(field(bytes, 0)?),
            b: i64::from_be_bytes(field(bytes, 4)?),
            v: 0,
        })
    }
}

fn arg(n: usize, default: i64) -> i64 {
    env::args()
        .nth(n)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain = i32::try_from(arg(1, 0))?;
    let seconds = arg(2, 10);
    let base = i32::try_from(arg(3, 2000))?;
    // XTypes 1.3 §7.6.3.1.2: XCDR = 0, XCDR2 = 2.
    let representation: i16 = if arg(4, 1) == 1 { 0 } else { 2 };
    let reliable = ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        ..Default::default()
    };
    let participant = DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())?;
    let topic = participant.create_topic::<Pair>("KhPair", TopicQos::default())?;
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Pair>(
            &topic,
            DataWriterQos {
                reliability: reliable,
                data_representation: Some(vec![representation]),
                ..Default::default()
            },
        )?;
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Pair>(
            &topic,
            DataReaderQos {
                reliability: reliable,
                history: HistoryQosPolicy {
                    kind: HistoryKind::KeepAll,
                    depth: 1,
                },
                data_representation: Some(vec![0, 2]),
                ..Default::default()
            },
        )?;
    println!(
        "zerodds writer KeyHash {:02x?}",
        Pair { a: 1, b: 2, v: 0 }.key_hash()
    );
    for tick in 0..seconds * 2 {
        writer.write(&Pair {
            a: 1,
            b: 2,
            v: base + i32::try_from(tick)?,
        })?;
        for sample in reader.take_with_info()? {
            if sample.info.valid_data {
                println!(
                    "zerodds-reader v={} a={} b={} instance={:?}",
                    sample.data.v, sample.data.a, sample.data.b, sample.info.instance_handle
                );
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}
