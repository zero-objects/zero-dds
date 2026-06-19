// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Shared helpers for the routing-service e2e tests. Each e2e test lives in its
//! OWN `tests/*.rs` file so cargo runs it in a SEPARATE process — full
//! isolation from the process-global DDS state (factory singleton, in-process
//! discovery registry, multicast) that otherwise cross-contaminates DDS tests
//! sharing one binary.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, dead_code)]

use std::time::{Duration, Instant};

use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, PublisherQos, RawBytes, SubscriberQos, TopicQos,
};
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::reliability::ReliabilityKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;

/// Reliable + KEEP_ALL writer QoS (the byte-path default the router matches).
pub fn writer_qos() -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::Volatile;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

/// Reliable + KEEP_ALL reader QoS.
pub fn reader_qos() -> DataReaderQos {
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.durability.kind = DurabilityKind::Volatile;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

/// A `RawBytes` publisher on `(domain, topic)`.
pub struct Pub {
    _participant: DomainParticipant,
    writer: zerodds_dcps::DataWriter<RawBytes>,
}

impl Pub {
    pub fn new(domain: i32, topic: &str) -> Self {
        let factory = DomainParticipantFactory::instance();
        let p = factory
            .create_participant(domain, DomainParticipantQos::default())
            .unwrap();
        let publisher = p.create_publisher(PublisherQos::default());
        let t = p
            .create_topic::<RawBytes>(topic, TopicQos::default())
            .unwrap();
        let writer = publisher
            .create_datawriter::<RawBytes>(&t, writer_qos())
            .unwrap();
        Self {
            _participant: p,
            writer,
        }
    }

    pub fn wait_matched(&self, n: usize, timeout: Duration) {
        let _ = self.writer.wait_for_matched_subscription(n, timeout);
    }

    pub fn write(&self, bytes: Vec<u8>) {
        self.writer.write(&RawBytes::new(bytes)).unwrap();
    }
}

/// A `RawBytes` subscriber on `(domain, topic)`. Collects until `expect`
/// distinct payloads arrive or the timeout elapses.
pub struct Sub {
    _participant: DomainParticipant,
    reader: zerodds_dcps::DataReader<RawBytes>,
}

impl Sub {
    pub fn new(domain: i32, topic: &str) -> Self {
        let factory = DomainParticipantFactory::instance();
        let p = factory
            .create_participant(domain, DomainParticipantQos::default())
            .unwrap();
        let sub = p.create_subscriber(SubscriberQos::default());
        let t = p
            .create_topic::<RawBytes>(topic, TopicQos::default())
            .unwrap();
        let reader = sub.create_datareader::<RawBytes>(&t, reader_qos()).unwrap();
        Self {
            _participant: p,
            reader,
        }
    }

    /// Collects until `expect` DISTINCT payloads arrive or `timeout` elapses.
    pub fn collect(&self, expect: usize, timeout: Duration) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut distinct = std::collections::BTreeSet::new();
        let deadline = Instant::now() + timeout;
        while distinct.len() < expect && Instant::now() < deadline {
            let _ = self.reader.wait_for_data(Duration::from_millis(200));
            if let Ok(samples) = self.reader.take_with_info() {
                for s in samples {
                    if s.info.valid_data {
                        distinct.insert(s.data.data.clone());
                        out.push(s.data.data);
                    }
                }
            }
        }
        out
    }

    /// Drains for `dur`, returning everything received (used to assert a loop
    /// does NOT amplify — count must stay bounded).
    pub fn drain_for(&self, dur: Duration) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let deadline = Instant::now() + dur;
        while Instant::now() < deadline {
            let _ = self.reader.wait_for_data(Duration::from_millis(100));
            if let Ok(samples) = self.reader.take_with_info() {
                for s in samples {
                    if s.info.valid_data {
                        out.push(s.data.data);
                    }
                }
            }
        }
        out
    }
}
