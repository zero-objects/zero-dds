// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A bidirectional bridge does not feed back on itself (README of this
//! crate, "Loop guard": input and output participants on a shared domain
//! are made mutually invisible).
//!
//! Two routes form a bidirectional bridge between two domains on the same
//! topic `Bi`: `fwd` (A to B) and `back` (B to A). Without the loop guard a
//! sample written on A would be forwarded to B by `fwd`, read there by
//! `back`, forwarded to A, and by `fwd` again, without end. With it, each
//! application sample crosses the bridge once: the reader on B sees the
//! three samples written, and draining past their delivery yields no
//! runaway re-forwarding.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{Pub, Sub};
use zerodds_routing_service::{Router, RouterConfig};

/// The two ends of the bridge, on domains leased for this run.
///
/// Leased and not fixed: the domain number is the UDP port
/// (RTPS 9.6.1.4.1), so a number written into this file is shared with
/// every other test binary that wrote the same one. A second process
/// publishing on the input domain looks exactly like the runaway this
/// test exists to catch — measured on the bench host: 8606 deliveries
/// for 3 samples, with no loop in the router at all.
fn config(input: i32, output: i32) -> String {
    format!(
        r#"{{
  "name": "e2e-bidir",
  "routes": [
    {{
      "name": "fwd",
      "input":  {{ "domain": {input}, "topic": "Bi", "type_name": "zerodds::RawBytes" }},
      "output": {{ "domain": {output}, "topic": "Bi", "type_name": "zerodds::RawBytes" }}
    }},
    {{
      "name": "back",
      "input":  {{ "domain": {output}, "topic": "Bi", "type_name": "zerodds::RawBytes" }},
      "output": {{ "domain": {input}, "topic": "Bi", "type_name": "zerodds::RawBytes" }}
    }}
  ]
}}"#
    )
}

#[test]
fn the_loop_guard_keeps_a_bidirectional_bridge_from_feeding_back() {
    let input = zerodds_test_domains::unique_domain();
    let output = zerodds_test_domains::unique_domain();
    let cfg = RouterConfig::from_json(&config(input, output)).unwrap();
    let router = Router::start(&cfg).unwrap();

    let sub = Sub::new(output, "Bi");
    let publisher = Pub::new(input, "Bi");
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
    // `fwd` output on domain B, so it forwarded nothing of the router's own.
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
