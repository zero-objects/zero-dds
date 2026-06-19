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
//! 2. On the Cyclone host (bench VM), `ddsperf -i 42 sub` starts the
//!    local `subscriber` with the liveliness listener enabled.
//! 3. Cyclone sends back its own WLP heartbeat (once it has
//!    discovered us via SPDP).
//! 4. We verify that the ZeroDDS runtime sees the Cyclone prefix as
//!    a peer in `peer_liveliness_last_seen()` — i.e., that the
//!    wire encoding is bidirectionally compatible.
//!
//! # Prerequisites
//!
//! - SSH access to the Linux bench host with the lab password (lab setup)
//! - `sshpass` installed locally
//! - Cyclone DDS 0.10.2 (`ddsperf` binary) on the bench host
//! - `/tmp/cyc.xml` on the bench host pins Cyclone to `enp6s18`
//! - Local host on the same LAN as the bench host, with multicast pass-through
//!
//! In CI the test stays ignored — it only runs manually on the
//! lab host.

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
use zerodds_rtps::wire_types::GuidPrefix;

#[test]
#[ignore = "live cyclone interop — opt-in via --ignored, requires lab setup"]
fn cyclone_live_wlp_handshake() {
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

    // Start the Cyclone subscriber (lab-specific — tooling comes
    // over the same ssh pattern as cyclone_live_sedp).
    eprintln!("ZeroDDS WLP endpoint live; waiting 5 s for Cyclone beacons");
    thread::sleep(Duration::from_secs(5));

    // We do not know the Cyclone prefix a priori — we simply check
    // whether the WLP endpoint has registered at least one peer.
    // If ddsperf is not running, the test fails — that is the
    // manual verification marked with `#[ignore]`.
    let count = rt.wlp.lock().ok().map(|w| w.peer_count()).unwrap_or(0);
    assert!(
        count > 0,
        "no Cyclone peer in the WLP cache — is ddsperf -D 42 sub running on the bench host?"
    );
}
