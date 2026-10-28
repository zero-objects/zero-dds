//! WP 1.D — live-interop test for the writer-liveliness protocol against
//! a real Cyclone DDS instance.
//!
//! **Opt-in only** — marked `#[ignore]`. Invocation:
//!
//! ```bash
//! cargo test -p zerodds-dcps --test cyclone_live_wlp -- --ignored --nocapture
//! ```
//!
//! # Test flow
//!
//! 1. A ZeroDDS runtime starts on domain 42, sending AUTOMATIC WLP
//!    heartbeats every 200 ms.
//! 2. The test starts its own local `ddsperf -i 42 sub` process.
//! 3. Cyclone wrote one ParticipantMessageData when its participant was
//!    created, before it knew ours, and announces it in the HEARTBEATs of
//!    its WLP writer (`first=1, last=1`); it writes no further pulse,
//!    since `ddsperf sub` has no writer with a finite liveliness lease.
//!    Measured 2026-09-30 with captures of domain 42: 11 to 13 such
//!    HEARTBEATs in five seconds per run and no second DATA.
//! 4. ZeroDDS's WLP reader asks for that pulse with an ACKNACK
//!    (`WlpEndpoint::heartbeat_responses`), Cyclone repairs it, and the
//!    participant Cyclone announced over SPDP appears in the WLP cache.
//!
//! Until the WLP reader answered HEARTBEATs this test passed only because
//! the cache counted ZeroDDS's own pulse, looped back by the multicast
//! group; with that skipped (2989d6c1ee0) it failed, and the capture above
//! showed why.
//!
//! # Prerequisites
//!
//! - Cyclone DDS `ddsperf` in PATH, or an explicit DDSPERF executable path
//! - Local multicast support
//! - INTEROP_REQUIRE=1 in the required CI gate
//!
//! The live-interop CI job explicitly executes this ignored target. Child logs
//! are retained and cleanup is restricted to the process started by this test.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::thread;
use std::time::Duration;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_rtps::wire_types::{GuidPrefix, VendorId};

#[path = "common/cross_vendor.rs"]
mod cross_vendor;

#[test]
#[ignore = "live cyclone interop — opt-in via --ignored, requires local ddsperf"]
fn cyclone_live_wlp_handshake() {
    if !cross_vendor::require_or_skip("cyclone_live_wlp") {
        return;
    }
    let mut cyclone =
        cross_vendor::start_cyclone_ddsperf_sub(42, 30).expect("start local Cyclone ddsperf");
    // Aggressive WLP period so that Cyclone sees us as alive within a
    // few seconds.
    let cfg = RuntimeConfig {
        tick_period: Duration::from_millis(50),
        spdp_period: Duration::from_millis(500),
        wlp_period: Duration::from_millis(200),
        participant_lease_duration: Duration::from_millis(600),
        ..RuntimeConfig::default()
    };
    let rt = DcpsRuntime::start(42, GuidPrefix::from_bytes([0xAA; 12]), cfg).expect("start");

    eprintln!("ZeroDDS WLP endpoint live; waiting up to 5 s for Cyclone's pulse");
    // The Cyclone participant SPDP announced, and whether the WLP cache
    // holds it. Polled, so a pass does not cost the whole window.
    let cyclone_in_wlp_cache = || {
        rt.discovered_participants()
            .iter()
            .filter(|p| p.sender_vendor == VendorId::CYCLONE)
            .any(|p| {
                rt.wlp
                    .lock()
                    .is_ok_and(|w| w.peer_state(&p.sender_prefix).is_some())
            })
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !cyclone_in_wlp_cache() && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    cyclone
        .ensure_running()
        .expect("Cyclone peer exited during WLP observation");
    let discovered = rt
        .discovered_participants()
        .iter()
        .filter(|p| p.sender_vendor == VendorId::CYCLONE)
        .count();
    assert!(
        cyclone_in_wlp_cache(),
        "no Cyclone participant in the WLP cache after 5 s ({discovered} discovered over SPDP)"
    );
}
