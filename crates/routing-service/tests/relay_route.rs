// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e: a relay route forwards on behalf of the writer (DDS-Security 1.2
//! §7.1.1.4, DDSI-RTPS 2.5 §8.7.9).
//!
//! A writer and a reader on one domain and topic, and a relay route on the
//! same domain and topic. The reader matches the writer and the router's
//! output writer, so it receives every sample twice: from the writer and
//! forwarded. The route forwards each sample on the writer's behalf, and the
//! reader takes it once, as the writer's, with the writer's sequence number.
//! Before, the route had no relay mode; a same-topic route on the same
//! domain delivered each sample a second time, as the router's.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_routing_service::{Router, RouterConfig};
use zerodds_rtps::wire_types::{Guid, GuidPrefix};

const TOPIC: &str = "Relayed";
const TYPE: &str = "zerodds::RawBytes";

#[test]
fn a_relay_route_forwards_on_behalf_of_the_writer() {
    let domain = zerodds_test_domains::unique_domain();
    let cfg = RouterConfig::from_json(&format!(
        r#"{{
  "name": "relay",
  "routes": [
    {{
      "name": "trent",
      "relay": true,
      "input":  {{ "domain": {domain}, "topic": "{TOPIC}", "type_name": "{TYPE}" }},
      "output": {{ "domain": {domain}, "topic": "{TOPIC}", "type_name": "{TYPE}" }}
    }}
  ]
}}"#
    ))
    .unwrap();
    let router = Router::start(&cfg).unwrap();

    let alice = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes([0xA1; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    let carol = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes([0xC3; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    let writer = alice
        .register_user_writer(UserWriterConfig {
            topic_name: TOPIC.into(),
            type_name: TYPE.into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let (reader, rx) = carol
        .register_user_reader(UserReaderConfig {
            topic_name: TOPIC.into(),
            type_name: TYPE.into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    // Carol's reader matches alice's writer and the route's output writer;
    // alice's writer matches carol's reader and the route's relay reader.
    let deadline = Instant::now() + Duration::from_secs(20);
    while carol.user_reader_matched_writer_guids(reader).len() < 2
        || alice.user_writer_matched_reader_guids(writer).len() < 2
    {
        assert!(Instant::now() < deadline, "the endpoints did not match");
        std::thread::sleep(Duration::from_millis(100));
    }

    let n = 10u8;
    for i in 0..n {
        alice.write_user_sample(writer, vec![i, 0x5a]).unwrap();
    }
    let alice_writer = Guid::new(GuidPrefix::from_bytes([0xA1; 12]), writer).to_bytes();
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(300)) {
            Ok(UserSample::Alive {
                payload,
                writer_guid,
                source_sequence_number,
                ..
            }) => {
                assert_eq!(writer_guid, alice_writer, "a sample of the router's own");
                seen.push((source_sequence_number, payload.as_ref().to_vec()));
            }
            Ok(other) => panic!("unexpected {other:?}"),
            Err(_) if seen.len() >= usize::from(n) => break,
            Err(_) => {}
        }
    }
    // Late copies would be duplicates.
    if let Ok(sample) = rx.recv_timeout(Duration::from_millis(1500)) {
        panic!("a second copy: {sample:?}");
    }
    seen.sort();
    let expected: Vec<(i64, Vec<u8>)> = (0..n).map(|i| (i64::from(i) + 1, vec![i, 0x5a])).collect();
    assert_eq!(seen, expected, "each sample once, as alice's");
    let metrics = router.route_metrics("trent").unwrap();
    assert_eq!(metrics.errors, 0);
    assert_eq!(
        metrics.forwarded,
        u64::from(n),
        "the route forwarded every sample, and carol dropped the copies"
    );
    alice.shutdown();
    carol.shutdown();
}
