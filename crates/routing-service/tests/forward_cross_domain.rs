// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e (v1): real DDS samples forward across domains through the router.
//!
//! An application writer on domain A, topic `SensorIn` → the router bridges to
//! domain B, topic `SensorOut` (domain bridge + topic rename) → an application
//! reader on domain B receives the verbatim CDR bodies. Own test binary =
//! isolated process.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{Pub, Sub};
use zerodds_routing_service::{Router, RouterConfig};

const CONFIG: &str = r#"{
  "name": "e2e-bridge",
  "routes": [
    {
      "name": "sensor",
      "input":  { "domain": 40, "topic": "SensorIn",  "type_name": "zerodds::RawBytes" },
      "output": { "domain": 41, "topic": "SensorOut", "type_name": "zerodds::RawBytes" }
    }
  ]
}"#;

#[test]
fn forwards_bytes_across_domains_with_topic_rename() {
    let cfg = RouterConfig::from_json(CONFIG).unwrap();
    let router = Router::start(&cfg).unwrap();
    assert_eq!(router.route_names(), vec!["sensor".to_string()]);

    // Downstream reader on the OUTPUT domain/topic, app writer on the INPUT.
    let sub = Sub::new(41, "SensorOut");
    let publisher = Pub::new(40, "SensorIn");

    // The writer matches the router's input reader; the router's output writer
    // matches `sub` — both legs must come up before we publish.
    publisher.wait_matched(1, Duration::from_secs(15));

    let payloads: Vec<Vec<u8>> = (0u8..5).map(|i| vec![i, 0xAA, 0xBB, i]).collect();
    for p in &payloads {
        publisher.write(p.clone());
    }

    let got = sub.collect(payloads.len(), Duration::from_secs(15));
    let mut got_sorted = got.clone();
    got_sorted.sort();
    let mut want = payloads.clone();
    want.sort();
    assert_eq!(
        got_sorted, want,
        "every body must arrive verbatim across the domain bridge (got {got:?})"
    );

    // Metrics must account for the forwarded samples. The pump increments the
    // counter on its own thread right after the write; poll until it settles to
    // the expected value (the increment is cross-thread of this assertion, not
    // delayed work — the reader already received every body above).
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut m = router.route_metrics("sensor").unwrap();
    while m.forwarded < payloads.len() as u64 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
        m = router.route_metrics("sensor").unwrap();
    }
    assert!(
        m.forwarded >= payloads.len() as u64,
        "forwarded counter {} < {}",
        m.forwarded,
        payloads.len()
    );
    assert_eq!(m.errors, 0, "no forward errors expected");
}
