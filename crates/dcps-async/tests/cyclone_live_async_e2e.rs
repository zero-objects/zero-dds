// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-async-1.0 §8 — an async reader against a Cyclone DDS writer.
//!
//! Cyclone's own `ddsperf` publishes its `OneULong` samples (`struct
//! OneULong { unsigned long seq; }`, `ddsperf -T OU`) on the topic
//! `DDSPerfRDataOU` of a leased domain (in the default partition: ddsperf
//! builds a `DDSPerf` partition QoS but creates its publisher without it); an
//! `AsyncDataReader` of this crate takes them through `take().await`. The
//! oracle is Cyclone: the samples it wrote arrive, their sequence numbers
//! rising.
//!
//! `#[ignore]` in the crate tier: it needs `ddsperf` (on `PATH`, or the
//! executable `DDSPERF` names). It runs in the job `vendor-live-dcps`
//! (`ci/interop-omg-weekly.yml`, the interop image with Cyclone).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DdsType, DecodeError, EncodeError};
use zerodds_dcps_async::{AsyncDomainParticipantFactory, DataReaderQos, SubscriberQos, TopicQos};

/// `struct OneULong { unsigned long seq; };` of Cyclone's
/// `ddsperf_types.idl`, `@final`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OneULong {
    seq: u32,
}

impl DdsType for OneULong {
    const TYPE_NAME: &'static str = "OneULong";
    /// One `unsigned long`: XCDR1 and XCDR2 write it the same way.
    const XCDR1_MATCHES_XCDR2: bool = true;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.seq.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let seq = bytes.get(..4).ok_or(DecodeError::Invalid {
            what: "OneULong: fewer than four octets",
        })?;
        Ok(Self {
            seq: u32::from_le_bytes(seq.try_into().unwrap()),
        })
    }

    fn decode_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let seq = bytes.get(..4).ok_or(DecodeError::Invalid {
            what: "OneULong: fewer than four octets",
        })?;
        Ok(Self {
            seq: u32::from_be_bytes(seq.try_into().unwrap()),
        })
    }
}

/// `ddsperf`, stopped with the test.
struct Ddsperf(Child);

impl Drop for Ddsperf {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[ignore = "needs Cyclone DDS's ddsperf (vendor-live-dcps)"]
#[tokio::test(flavor = "multi_thread")]
async fn async_8_an_async_reader_takes_the_samples_of_a_cyclone_writer() {
    let lease = zerodds_test_domains::lease_domain();
    let participant = AsyncDomainParticipantFactory::instance()
        .create_participant(lease.id())
        .expect("participant");
    let topic = participant
        .create_topic::<OneULong>("DDSPerfRDataOU", TopicQos::default())
        .expect("topic");
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<OneULong>(&topic, DataReaderQos::default())
        .expect("reader");

    let program = std::env::var("DDSPERF").unwrap_or_else(|_| "ddsperf".into());
    let _ddsperf = Ddsperf(
        Command::new(&program)
            .args([
                "-i",
                &lease.id().to_string(),
                "-T",
                "OU",
                "-D",
                "15",
                "pub",
                "20Hz",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("{program}: {e}")),
    );

    let mut seqs = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(15);
    while seqs.len() < 5 {
        assert!(
            Instant::now() < deadline,
            "{} samples of ddsperf arrived in 15 s: {seqs:?}",
            seqs.len()
        );
        match reader.take(Duration::from_millis(500)).await {
            Ok(samples) => seqs.extend(samples.into_iter().map(|s| s.seq)),
            Err(zerodds_dcps_async::DdsError::Timeout) => {}
            Err(e) => panic!("take: {e:?}"),
        }
    }
    assert!(
        seqs.windows(2).all(|w| w[0] < w[1]),
        "the sequence numbers ddsperf wrote rise: {seqs:?}"
    );
}
