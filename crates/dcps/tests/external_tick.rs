// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-async-1.0 §4 — external tick driver. Proves that
//! `RuntimeConfig::external_tick` suppresses the dedicated `zdds-tick` thread
//! (so `tick_count()` stays put on its own) and that the `DcpsTickDriver`
//! obtained via `DcpsRuntime::tick_driver()` advances the very same tick — i.e.
//! an external executor (tokio's `spawn_in_tokio`) drives discovery/deadline/
//! lifespan/liveliness work exactly as the internal thread would.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_rtps::wire_types::GuidPrefix;

#[test]
fn external_tick_suppresses_thread_and_driver_advances() {
    let rt = DcpsRuntime::start(
        61,
        GuidPrefix::from_bytes([0x65; 12]),
        RuntimeConfig {
            external_tick: true,
            ..RuntimeConfig::default()
        },
    )
    .expect("start runtime with external_tick");

    // No internal `zdds-tick` thread: the count must NOT advance on its own,
    // even after several tick periods elapse.
    let before = rt.tick_count();
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(
        rt.tick_count(),
        before,
        "external_tick=true must not spawn the internal tick thread"
    );

    // The external driver advances the exact same tick.
    let mut driver = rt.tick_driver();
    assert!(!driver.is_stopped());
    assert!(driver.tick_period() > Duration::ZERO);
    for _ in 0..5 {
        driver.tick();
    }
    assert_eq!(
        rt.tick_count(),
        before + 5,
        "each DcpsTickDriver::tick() runs exactly one iteration"
    );
}

#[test]
fn internal_tick_thread_advances_on_its_own() {
    // Control case: the default (external_tick=false) keeps the dedicated
    // thread, so the count rises without any manual driving.
    let rt = DcpsRuntime::start(
        62,
        GuidPrefix::from_bytes([0x66; 12]),
        RuntimeConfig {
            tick_period: Duration::from_millis(5),
            ..RuntimeConfig::default()
        },
    )
    .expect("start runtime with internal tick");

    let before = rt.tick_count();
    std::thread::sleep(Duration::from_millis(120));
    assert!(
        rt.tick_count() > before,
        "internal zdds-tick thread should advance the tick count on its own"
    );
}
