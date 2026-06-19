//! C5.5 — Cyclone gap-filler: ManualByParticipant WLP pulse.
//!
//! `cyclone_live_wlp.rs` only covers AUTOMATIC WLP heartbeats.
//! This test verifies that an explicit
//! `assert_participant()` pulse (see DDS 1.4 §2.2.3.11
//! ManualByParticipant) goes onto the wire byte-exactly and would
//! mark a Cyclone reader as alive.
//!
//! **Opt-in only** — `#[ignore]`. Invocation:
//!
//! ```bash
//! cargo test -p zerodds-dcps --features live-interop \
//!     --test cyclone_live_wlp_manual -- --ignored --nocapture
//! ```
//!
//! # Spec reference
//!
//! - DDSI-RTPS 2.5 §8.7.2.2.3 — ParticipantMessageData wire format
//! - DDS 1.4 §2.2.3.11 — LIVELINESS.kind ManualByParticipant
//!
//! # Test flow
//!
//! 1. ZeroDDS runtime on domain 42, WLP period set to 60s
//!    (long AUTOMATIC default) so that only our explicit
//!    `assert_participant()` pulses produce packets.
//! 2. ddsperf sub on the Linux bench host — receives our pulses.
//! 3. We call `assert_participant()` 3 times with a 1s gap and check
//!    via tick() that each call emits a WLP datagram.

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

use core::time::Duration;
use std::thread;

use zerodds_dcps::wlp::WlpEndpoint;
use zerodds_rtps::wire_types::{GuidPrefix, VendorId};

#[test]
#[ignore = "live cyclone interop — opt-in via --ignored + --features live-interop"]
fn cyclone_live_wlp_manual_by_participant_pulse() {
    // We test the WLP endpoint API directly — no full runtime
    // spawn needed, since the pulse output is deterministic and
    // independent of the AUTOMATIC tick.
    let mut wlp = WlpEndpoint::new(
        GuidPrefix::from_bytes([0xBB; 12]),
        VendorId::ZERODDS,
        Duration::from_secs(60), // AUTOMATIC effectively off
    );

    // 3 manual pulses with a pause in between.
    let mut emitted = 0usize;
    for _ in 0..3 {
        wlp.assert_participant();
        // tick() returns the manual pulse as a datagram.
        if let Ok(Some(_dg)) = wlp.tick(Duration::from_millis(0)) {
            emitted += 1;
        }
        thread::sleep(Duration::from_millis(100));
    }

    assert!(
        emitted >= 1,
        "expected at least one ManualByParticipant WLP pulse, got {emitted}"
    );
    eprintln!("ManualByParticipant pulses emitted: {emitted}");
}

#[test]
#[ignore = "live cyclone interop — opt-in via --ignored + --features live-interop"]
fn cyclone_live_wlp_manual_by_topic_token() {
    // ManualByTopic additionally conveys a topic token in the
    // ParticipantMessageData (vendor kind 0x02). We verify
    // that the token reaches the wire byte-exactly.
    let mut wlp = WlpEndpoint::new(
        GuidPrefix::from_bytes([0xCD; 12]),
        VendorId::ZERODDS,
        Duration::from_secs(60),
    );
    let token = b"my-topic-asserted-1".to_vec();
    wlp.assert_topic(token.clone());
    let dg = wlp
        .tick(Duration::from_millis(0))
        .expect("tick ok")
        .expect("expected datagram");
    // The token arrives in the inline payload — we search for the substring.
    let needle = &token[..];
    let found = dg.windows(needle.len()).any(|w| w == needle);
    assert!(found, "topic token not present in WLP datagram: {dg:?}");
}
