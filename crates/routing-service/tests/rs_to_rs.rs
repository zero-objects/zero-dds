// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e (O1): two routing services chained — RS↔RS interop. Router A bridges
//! domain 44 → 45, router B bridges 45 → 46, each renaming the topic per hop.
//! A sample published on domain 44 must arrive on domain 46 through both
//! routers. Own test binary = isolated process.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{Pub, Sub};
use zerodds_routing_service::{Router, RouterConfig};

const ROUTER_A: &str = r#"{
  "name": "rs-a",
  "routes": [
    {
      "name": "a",
      "input":  { "domain": 44, "topic": "SensorIn",  "type_name": "zerodds::RawBytes" },
      "output": { "domain": 45, "topic": "SensorMid", "type_name": "zerodds::RawBytes" }
    }
  ]
}"#;

const ROUTER_B: &str = r#"{
  "name": "rs-b",
  "routes": [
    {
      "name": "b",
      "input":  { "domain": 45, "topic": "SensorMid", "type_name": "zerodds::RawBytes" },
      "output": { "domain": 46, "topic": "SensorOut", "type_name": "zerodds::RawBytes" }
    }
  ]
}"#;

#[test]
fn sample_flows_through_two_chained_routers() {
    let router_a = Router::start(&RouterConfig::from_json(ROUTER_A).unwrap()).unwrap();
    let router_b = Router::start(&RouterConfig::from_json(ROUTER_B).unwrap()).unwrap();
    assert_eq!(router_a.route_names(), vec!["a".to_string()]);
    assert_eq!(router_b.route_names(), vec!["b".to_string()]);

    // Reader at the far end (domain 46), writer at the near end (domain 44).
    let sub = Sub::new(46, "SensorOut");
    let publisher = Pub::new(44, "SensorIn");
    // The full chain must come up: writer→A.in, A.out→B.in, B.out→sub.
    publisher.wait_matched(1, Duration::from_secs(20));

    let payloads: Vec<Vec<u8>> = (0u8..5).map(|i| vec![i, 0x44, 0x46, i]).collect();
    for p in &payloads {
        publisher.write(p.clone());
    }

    let got = sub.collect(payloads.len(), Duration::from_secs(20));
    let mut got_sorted = got.clone();
    got_sorted.sort();
    let mut want = payloads.clone();
    want.sort();
    assert_eq!(
        got_sorted, want,
        "every body must traverse both routers (44→45→46), got {got:?}"
    );
}
