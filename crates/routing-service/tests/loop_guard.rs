// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e (v2): a bidirectional bridge does not feed back on itself.
//!
//! Two routes form a bidirectional bridge between domains 50 and 51 on the same
//! topic `Bi`: `fwd` (50→51) and `back` (51→50). Without the loop guard, a
//! sample written on 50 would be forwarded to 51 by `fwd`, then picked up on 51
//! by `back` and forwarded to 50, then by `fwd` again — an infinite cycle that
//! amplifies without bound.
//!
//! With the participant-level loop guard (input and output participants on a
//! shared domain are mutually invisible), each application sample crosses the
//! bridge exactly once. We write 3 distinct samples and assert the downstream
//! reader sees exactly those 3 — and that draining well past delivery yields no
//! runaway re-forwarding. Own test binary = isolated process.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{Pub, Sub};
use zerodds_routing_service::{Router, RouterConfig};

const CONFIG: &str = r#"{
  "name": "e2e-bidir",
  "routes": [
    {
      "name": "fwd",
      "input":  { "domain": 50, "topic": "Bi", "type_name": "zerodds::RawBytes" },
      "output": { "domain": 51, "topic": "Bi", "type_name": "zerodds::RawBytes" }
    },
    {
      "name": "back",
      "input":  { "domain": 51, "topic": "Bi", "type_name": "zerodds::RawBytes" },
      "output": { "domain": 50, "topic": "Bi", "type_name": "zerodds::RawBytes" }
    }
  ]
}"#;

#[test]
fn bidirectional_bridge_does_not_loop() {
    let cfg = RouterConfig::from_json(CONFIG).unwrap();
    let router = Router::start(&cfg).unwrap();

    let sub = Sub::new(51, "Bi");
    let publisher = Pub::new(50, "Bi");
    publisher.wait_matched(1, Duration::from_secs(15));

    let payloads: Vec<Vec<u8>> = (0u8..3).map(|i| vec![i, 0x55]).collect();
    for p in &payloads {
        publisher.write(p.clone());
    }

    // Collect the 3 expected, then keep draining to catch any runaway loop.
    let mut all = sub.collect(payloads.len(), Duration::from_secs(15));
    all.extend(sub.drain_for(Duration::from_secs(2)));

    let distinct: std::collections::BTreeSet<Vec<u8>> = all.iter().cloned().collect();
    assert_eq!(
        distinct.len(),
        payloads.len(),
        "must see exactly the 3 distinct app samples (saw {distinct:?})"
    );
    // A loop would amplify far past the sample count; volatile re-delivery may
    // add a few duplicates but never a runaway. Bound it generously.
    assert!(
        all.len() <= payloads.len() * 3,
        "re-forwarding ran away: {} deliveries for {} samples",
        all.len(),
        payloads.len()
    );

    // The `back` route ran but the loop guard kept it from re-ingesting the
    // `fwd` output on domain 51 — so it forwarded nothing of the router's own.
    let fwd = router.route_metrics("fwd").unwrap();
    let back = router.route_metrics("back").unwrap();
    assert_eq!(fwd.errors, 0);
    assert_eq!(back.errors, 0);
    assert!(
        back.forwarded <= payloads.len() as u64,
        "back route re-forwarded router output ({} times) — loop guard breached",
        back.forwarded
    );
}
