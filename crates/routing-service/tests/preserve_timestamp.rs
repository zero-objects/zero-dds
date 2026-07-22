// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e (O1): a route with `preserve_source_timestamp: true` is accepted (it used
//! to be rejected as unsupported) and forwards samples through the
//! source-timestamp-preserving write path. An application writer on domain 42
//! → the router bridges to domain 43 → an application reader receives the
//! verbatim bodies, stamped with the input's source timestamp instead of the
//! router's own wall clock. Own test binary = isolated process.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{Pub, Sub};
use zerodds_routing_service::{Router, RouterConfig};

const CONFIG: &str = r#"{
  "name": "preserve-ts-bridge",
  "routes": [
    {
      "name": "sensor",
      "input":  { "domain": 42, "topic": "SensorIn",  "type_name": "zerodds::RawBytes" },
      "output": { "domain": 43, "topic": "SensorOut", "type_name": "zerodds::RawBytes" },
      "preserve_source_timestamp": true
    }
  ]
}"#;

#[test]
fn preserve_source_timestamp_route_starts_and_forwards() {
    // Previously `Router::start` returned a Config error for any route with
    // preserve_source_timestamp; now the source timestamp is threaded on
    // UserSample::Alive and forwarded via write_user_sample_stamped, so the
    // route must start and forward.
    let cfg = RouterConfig::from_json(CONFIG).unwrap();
    let router = Router::start(&cfg).unwrap();
    assert_eq!(router.route_names(), vec!["sensor".to_string()]);

    let sub = Sub::new(43, "SensorOut");
    let publisher = Pub::new(42, "SensorIn");
    publisher.wait_matched(1, Duration::from_secs(15));

    let payloads: Vec<Vec<u8>> = (0u8..5).map(|i| vec![i, 0xC0, 0xDE, i]).collect();
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
        "preserve_source_timestamp route must forward every body (got {got:?})"
    );
}
