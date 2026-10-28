// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `DcpsRuntime::wait_discovery_change` wakes when a remote endpoint is
//! discovered, so a caller that watches discovery (the Zenoh bridge's
//! auto-discovery) reacts to the SEDP datagram instead of a poll tick.
//!
//! Linux only: multicast loopback on macOS is unreliable for SPDP.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
use zerodds_rtps::wire_types::GuidPrefix;

const TOPIC: &str = "DiscoveryGeneration";

fn start(domain: i32, prefix: u8) -> std::sync::Arc<DcpsRuntime> {
    DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes([prefix; 12]),
        RuntimeConfig::default(),
    )
    .unwrap()
}

fn discovered(rt: &DcpsRuntime) -> bool {
    rt.discovered_publication_endpoints()
        .iter()
        .any(|ep| ep.topic_name == TOPIC)
}

#[test]
fn a_remote_writer_wakes_a_discovery_waiter() {
    let domain = common::unique_domain();
    let watcher = start(domain, 0xC1);
    let producer = start(domain, 0xC2);
    let first = watcher.discovery_generation();

    // A waiter parked before the writer exists.
    let waiter = {
        let watcher = std::sync::Arc::clone(&watcher);
        std::thread::spawn(move || {
            let started = Instant::now();
            let generation = watcher.wait_discovery_change(first, Duration::from_secs(10));
            (generation, started.elapsed())
        })
    };
    producer
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: TOPIC.into(),
                type_name: "T".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let (generation, waited) = waiter.join().unwrap();
    assert_ne!(generation, first, "discovery moved no generation");
    assert!(
        waited < Duration::from_secs(5),
        "the waiter woke after {waited:?}, not on the discovery"
    );
    assert!(
        discovered(&watcher),
        "woken, but the writer is not discovered"
    );

    // A change that happened before the wait starts is not lost.
    let started = Instant::now();
    let now_gen = watcher.wait_discovery_change(first, Duration::from_secs(10));
    assert_ne!(now_gen, first);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "a wait on a stale generation blocked for {:?}",
        started.elapsed()
    );
}
