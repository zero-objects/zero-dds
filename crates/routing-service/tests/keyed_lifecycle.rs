// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e (v1, keyed): dispose lifecycle propagation across domains.
//!
//! For a keyed topic the router must forward not only ALIVE samples but the
//! instance lifecycle (`DISPOSED` / `UNREGISTERED`, Spec §9.6.3.9 PID_STATUS_-
//! INFO). An application writer on domain 30 writes a keyed instance and then
//! disposes it; the router forwards both the data and the dispose to domain 31,
//! where the reader observes the instance transition to `NOT_ALIVE_DISPOSED`
//! (a sample with `valid_data == false`). Own test binary = isolated process.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipantFactory, DomainParticipantQos,
    InstanceStateKind, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::reliability::ReliabilityKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;
use zerodds_routing_service::{Router, RouterConfig};

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

/// The bridge config, on domains leased for this run.
///
/// Leased and not fixed: the domain number is the UDP port
/// (RTPS 9.6.1.4.1), so a number written into this file is shared with
/// every other test binary that wrote the same one, and the samples of
/// both arrive at this reader.
fn config(input: i32, output: i32) -> String {
    format!(
        r#"{{
  "name": "e2e-keyed",
  "routes": [
    {{
      "name": "keyed",
      "input":  {{ "domain": {input}, "topic": "Inst", "type_name": "test::KeyedRecord", "keyed": true }},
      "output": {{ "domain": {output}, "topic": "Inst", "type_name": "test::KeyedRecord", "keyed": true }}
    }}
  ]
}}"#
    )
}

fn writer_qos() -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

fn reader_qos() -> DataReaderQos {
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepAll;
    q.resource_limits.max_samples = LENGTH_UNLIMITED;
    q
}

#[test]
fn dispose_lifecycle_forwards_across_domains() {
    let input = zerodds_test_domains::unique_domain();
    let output = zerodds_test_domains::unique_domain();
    let cfg = RouterConfig::from_json(&config(input, output)).unwrap();
    let router = Router::start(&cfg).unwrap();

    let factory = DomainParticipantFactory::instance();

    // Downstream (output) reader on domain 31.
    let pr = factory
        .create_participant(output, DomainParticipantQos::default())
        .unwrap();
    let sub = pr.create_subscriber(SubscriberQos::default());
    let rtopic = pr
        .create_topic::<KeyedRecord>("Inst", TopicQos::default())
        .unwrap();
    let reader = sub
        .create_datareader::<KeyedRecord>(&rtopic, reader_qos())
        .unwrap();

    // Application writer on domain 30.
    let pw = factory
        .create_participant(input, DomainParticipantQos::default())
        .unwrap();
    let publisher = pw.create_publisher(PublisherQos::default());
    let wtopic = pw
        .create_topic::<KeyedRecord>("Inst", TopicQos::default())
        .unwrap();
    let writer = publisher
        .create_datawriter::<KeyedRecord>(&wtopic, writer_qos())
        .unwrap();

    writer
        .wait_for_matched_subscription(1, Duration::from_secs(15))
        .expect("writer matches router input reader");

    // Write the instance, then dispose it.
    let inst = KeyedRecord { id: 7, value: 42 };
    let handle = writer.register_instance(&inst).unwrap();
    writer.write(&inst).unwrap();

    // Wait for the ALIVE sample to land downstream before disposing, so the
    // instance exists on the reader when the dispose arrives.
    let mut saw_alive = false;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        let _ = reader.wait_for_data(Duration::from_millis(200));
        if let Ok(samples) = reader.read_with_info() {
            if samples.iter().any(|s| s.info.valid_data) {
                saw_alive = true;
                break;
            }
        }
    }
    assert!(
        saw_alive,
        "ALIVE sample must forward to the downstream reader"
    );

    writer.dispose(&inst, handle).unwrap();

    // The dispose marker must forward and flip the instance state downstream.
    let mut saw_disposed = false;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        let _ = reader.wait_for_data(Duration::from_millis(200));
        if let Ok(samples) = reader.take_with_info() {
            if samples
                .iter()
                .any(|s| s.info.instance_state == InstanceStateKind::NotAliveDisposed)
            {
                saw_disposed = true;
                break;
            }
        }
    }
    assert!(
        saw_disposed,
        "the dispose lifecycle must forward across the domain bridge (instance must read \
         NOT_ALIVE_DISPOSED downstream)"
    );

    // The pump increments the lifecycle counter on its own thread right after
    // the forward; poll until it settles (cross-thread of this assertion — the
    // dispose was already observed downstream above).
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut m = router.route_metrics("keyed").unwrap();
    while m.lifecycle < 1 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
        m = router.route_metrics("keyed").unwrap();
    }
    assert!(m.lifecycle >= 1, "lifecycle counter must count the forward");
    assert_eq!(m.errors, 0);
}
