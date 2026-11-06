// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C3 WiFi-robust discovery — the initial-announcement burst
//! (`RuntimeConfig::initial_announce_count` / `initial_announce_period`).
//!
//! A fresh participant must announce SPDP at the *fast* burst cadence while no
//! peer has been discovered yet, then fall back to the slow `spdp_period`. Over
//! a lossy / power-saving WiFi link a single startup beacon + 5 s period leaves
//! `participants=0` when the first beacons drop in the cold-start window; the
//! burst keeps the NIC awake, holds the stateful-firewall pinhole open and
//! elicits directed responses inside the wake windows (analogous to Fast DDS
//! `initial_announcements`).
//!
//! These tests run on loopback (no loss) and exploit the fact that a *peer-less*
//! participant keeps bursting up to `initial_announce_count`, so the cadence is
//! observable via `DcpsRuntime::spdp_announce_count()` without needing a lossy
//! link.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stderr,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::time::Duration;

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};

/// A peer-less participant emits the fast initial-announcement burst, then stops
/// (the long `spdp_period` keeps it quiet afterwards). With the burst disabled
/// (`initial_announce_count = 0`) it falls back to the legacy behaviour: just
/// the startup announce, then silence until the (long) `spdp_period`. Both
/// participants run side by side on domains of their own, so the window is
/// waited once.
#[test]
fn fresh_participant_bursts_then_slows() {
    let factory = DomainParticipantFactory::instance();
    // 8 announces @ 100 ms (~700 ms of burst), then a 30 s period — so after the
    // burst nothing more fires inside the measurement window.
    let bursting = RuntimeConfig {
        spdp_period: Duration::from_secs(30),
        initial_announce_count: 8,
        initial_announce_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    };
    let quiet = RuntimeConfig {
        spdp_period: Duration::from_secs(30),
        initial_announce_count: 0,
        ..RuntimeConfig::default()
    };
    let p = factory
        .create_participant_with_config(
            common::unique_domain(),
            DomainParticipantQos::default(),
            bursting,
        )
        .expect("participant");
    let q = factory
        .create_participant_with_config(
            common::unique_domain(),
            DomainParticipantQos::default(),
            quiet,
        )
        .expect("participant without a burst");
    let rt = p.runtime().expect("runtime").clone();
    let quiet_rt = q.runtime().expect("runtime").clone();

    // After ~1.2 s the 8-announce burst is complete and the 30 s period has not
    // re-fired. Allow scheduling jitter on both ends.
    std::thread::sleep(Duration::from_millis(1200));
    let n = rt.spdp_announce_count();
    eprintln!("[burst] peer-less announces in 1.2 s = {n} (expected ~8)");
    assert!(
        (5..=9).contains(&n),
        "fast burst should emit ~8 announces in 1.2 s, saw {n}"
    );
    let n = quiet_rt.spdp_announce_count();
    eprintln!("[burst] burst-disabled announces in 1.2 s = {n} (expected <=1)");
    assert!(
        n <= 1,
        "no burst → only the startup announce in 1.2 s (30 s period), saw {n}"
    );
}

/// Leases a DDS domain no other process on this host holds — the
/// domain number is the UDP port number, so it is a host-wide resource.
#[path = "common/mod.rs"]
mod common;
