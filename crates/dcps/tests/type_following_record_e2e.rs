// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! End-to-end for the **`zerodds-record` capture path** with type-following.
//!
//! Drives the exact pipeline `run_record` performs, over two real DcpsRuntime
//! on loopback SEDP:
//!   1. a typed writer (`cuas::Track`) publishes N samples;
//!   2. the recorder resolves the writer's REAL type via discovery (the settle
//!      phase), attaches a type-following opaque reader, and pumps every
//!      received sample into a `RecordingSession`;
//!   3. the `.zddsrec` bytes are read back and asserted: header carries the
//!      real type (`cuas::Track`, NOT RawBytes) and every frame's payload
//!      survives.
//!
//! This is the recorder analogue of `type_following_e2e.rs` and proves the
//! reported "record captures nothing / exits" is fixed: a RawBytes reader would
//! match no typed writer, so without type-following the session would record 0
//! frames.
//!
//! Linux-only (macOS multicast loopback is unreliable for SPDP). Run on codepit.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
};
use zerodds_recorder::format::{ParticipantEntry, SampleKind};
use zerodds_recorder::reader::RecordReader;
use zerodds_recorder::session::{RecordingSession, SessionOptions, TopicKey};
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

const TOPIC: &str = "Track";
const TYPED: &str = "cuas::Track";

/// In-memory sink we can read back after recording.
#[derive(Clone)]
struct SharedSink(Arc<Mutex<Vec<u8>>>);
impl Write for SharedSink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn writer_config(topic: &str, type_name: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: type_name.into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    }
}

fn reader_config(topic: &str, type_name: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: type_name.into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    }
}

fn wait_for_peers(rt: &Arc<DcpsRuntime>, n: usize, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if rt.discovered_participants().len() >= n {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn wait_for_reader_matched(rt: &Arc<DcpsRuntime>, eid: EntityId, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if rt.user_reader_matched_count(eid) >= 1 {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// The recorder's settle phase: resolve the real type for `topic` via discovery.
fn resolve_type(rt: &Arc<DcpsRuntime>, topic: &str, timeout: Duration) -> Option<String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some((_, ty)) = rt
            .discovered_publication_topics()
            .into_iter()
            .find(|(t, _)| t == topic)
        {
            return Some(ty);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn record_captures_typed_topic_via_type_following() {
    let domain = 143;
    let prefix_a = GuidPrefix::from_bytes([0xA1; 12]);
    let prefix_b = {
        let mut p = [0xB2; 12];
        p[..4].copy_from_slice(&prefix_a.to_bytes()[..4]);
        GuidPrefix::from_bytes(p)
    };
    let rt_a = DcpsRuntime::start(domain, prefix_a, RuntimeConfig::default()).expect("rt_a");
    let rt_b = DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b");
    assert!(
        wait_for_peers(&rt_a, 1, Duration::from_secs(10)),
        "rt_a !see rt_b"
    );
    assert!(
        wait_for_peers(&rt_b, 1, Duration::from_secs(10)),
        "rt_b !see rt_a"
    );

    let writer_eid = rt_a
        .register_user_writer(writer_config(TOPIC, TYPED))
        .unwrap();

    // Settle: the recorder learns the REAL type before freezing the header.
    let resolved = resolve_type(&rt_b, TOPIC, Duration::from_secs(10))
        .expect("recorder settle did not discover the writer's type");
    assert_eq!(resolved, TYPED, "settle must resolve the real writer type");

    // Build the session against the real type + attach a type-following reader.
    let buf = Arc::new(Mutex::new(Vec::new()));
    let opts = SessionOptions::new(1_000)
        .with_participant(ParticipantEntry {
            guid: [0xBB; 16],
            name: "rec".into(),
        })
        .with_topic(TopicKey {
            topic: TOPIC.into(),
            type_name: resolved.clone(),
        });
    let session = RecordingSession::new(SharedSink(Arc::clone(&buf)), opts);

    let (reader_eid, rx) = rt_b
        .register_user_reader(reader_config(TOPIC, &resolved))
        .unwrap();
    assert!(
        wait_for_reader_matched(&rt_b, reader_eid, Duration::from_secs(10)),
        "type-following recorder reader did not match the typed writer"
    );

    // Publish N samples; pump each received sample into the session (what
    // run_record's drain loop does).
    let key = TopicKey {
        topic: TOPIC.into(),
        type_name: resolved.clone(),
    };
    let n = 6usize;
    let mut captured = 0usize;
    for i in 0..n as u8 {
        let payload = vec![i; 8 + i as usize];
        rt_a.write_user_sample(writer_eid, payload.clone()).unwrap();
        if let Ok(UserSample::Alive {
            payload: got,
            writer_guid,
            ..
        }) = rx.recv_timeout(Duration::from_secs(2))
        {
            assert_eq!(got.as_slice(), payload.as_slice());
            session
                .record_sample(
                    2_000 + i64::from(i),
                    writer_guid,
                    &key,
                    SampleKind::Alive,
                    got.to_vec(),
                )
                .unwrap();
            captured += 1;
        }
    }
    assert_eq!(
        captured, n,
        "recorder did not capture every published sample"
    );
    assert_eq!(session.stats().samples_total as usize, n);

    // Read the .zddsrec back: real type in the header + all payloads intact.
    let bytes = buf.lock().unwrap().clone();
    let mut rdr = RecordReader::new(&bytes);
    let header = rdr.parse_header().unwrap();
    assert_eq!(header.topics.len(), 1);
    assert_eq!(header.topics[0].name, TOPIC);
    assert_eq!(
        header.topics[0].type_name, TYPED,
        "header must carry the real type, not RawBytes"
    );

    let mut read_back = 0usize;
    while let Some(frame) = rdr.next_frame().unwrap() {
        assert_eq!(frame.sample_kind, SampleKind::Alive);
        assert_eq!(frame.topic_idx, 0);
        let i = read_back as u8;
        assert_eq!(
            frame.payload,
            vec![i; 8 + i as usize],
            "frame {read_back} payload mismatch"
        );
        read_back += 1;
    }
    assert_eq!(read_back, n, ".zddsrec did not replay every captured frame");
}
