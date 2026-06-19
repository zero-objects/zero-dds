// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e (v3): content filtering across the wire.
//!
//! The route carries a flat `@final` struct `{ uint32 v }` (a representation-
//! invariant shape: XCDR1 and XCDR2 both encode it as 4 little-endian bytes
//! with no DHEADER, so the test does not depend on which representation the
//! source writer advertises). A SQL filter `v > 50` is applied per sample: the
//! router decodes the body against the type shape, evaluates the predicate,
//! and forwards only the matches — re-encoded — to the output domain.
//!
//! An app writer on domain 60 sends six values; the reader on domain 61 must
//! receive exactly the four with `v > 50`. Own test binary = isolated process.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{Pub, Sub};
use zerodds_routing_service::{Router, RouterConfig, TypeRegistry};

const CONFIG: &str = r#"{
  "name": "e2e-filter",
  "routes": [
    {
      "name": "hot",
      "input":  { "domain": 60, "topic": "Temp", "type_name": "zerodds::RawBytes" },
      "output": { "domain": 61, "topic": "Temp", "type_name": "zerodds::RawBytes" },
      "filter": { "expression": "v > 50" }
    }
  ]
}"#;

const SHAPES: &str = r#"[
  { "name": "zerodds::RawBytes", "members": [ { "name": "v", "kind": "u32" } ] }
]"#;

fn u32_body(v: u32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn body_u32(b: &[u8]) -> u32 {
    let mut a = [0u8; 4];
    a.copy_from_slice(&b[..4]);
    u32::from_le_bytes(a)
}

#[test]
fn sql_filter_drops_non_matching_samples() {
    let cfg = RouterConfig::from_json(CONFIG).unwrap();
    let shapes = TypeRegistry::from_json(SHAPES).unwrap();
    let router = Router::start_with_types(&cfg, shapes).unwrap();

    let sub = Sub::new(61, "Temp");
    let publisher = Pub::new(60, "Temp");
    publisher.wait_matched(1, Duration::from_secs(15));

    let values: [u32; 6] = [10, 60, 30, 80, 55, 100];
    for v in values {
        publisher.write(u32_body(v));
    }

    let want: std::collections::BTreeSet<u32> = values.into_iter().filter(|v| *v > 50).collect();
    let got_bodies = sub.collect(want.len(), Duration::from_secs(15));
    let got: std::collections::BTreeSet<u32> = got_bodies.iter().map(|b| body_u32(b)).collect();

    assert_eq!(
        got, want,
        "only samples with v > 50 must cross the filter (got {got:?})"
    );

    // The pump increments per-route counters on its own thread; poll until they
    // settle to the expected values (the increment is cross-thread of this
    // assertion, not delayed work).
    let dropped = (values.len() - want.len()) as u64;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut m = router.route_metrics("hot").unwrap();
    while (m.forwarded != want.len() as u64 || m.dropped_filter != dropped)
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(20));
        m = router.route_metrics("hot").unwrap();
    }
    assert_eq!(m.forwarded, want.len() as u64, "forwarded count");
    assert_eq!(m.dropped_filter, dropped, "dropped-by-filter count");
    assert_eq!(m.errors, 0);
}
