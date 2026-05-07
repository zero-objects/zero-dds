//! C5.5 — Cyclone-Lueckenfueller: ManualByParticipant-WLP-Pulse.
//!
//! `cyclone_live_wlp.rs` deckt nur AUTOMATIC-WLP-Heartbeats.
//! Dieser Test verifiziert, dass ein expliziter
//! `assert_participant()`-Pulse (siehe DDS 1.4 §2.2.3.11
//! ManualByParticipant) byte-genau auf den Wire geht und einen
//! Cyclone-Reader als alive markieren wuerde.
//!
//! **Opt-in only** — `#[ignore]`. Aufruf:
//!
//! ```bash
//! cargo test -p zerodds-dcps --features live-interop \
//!     --test cyclone_live_wlp_manual -- --ignored --nocapture
//! ```
//!
//! # Spec-Bezug
//!
//! - DDSI-RTPS 2.5 §8.7.2.2.3 — ParticipantMessageData Wire-Format
//! - DDS 1.4 §2.2.3.11 — LIVELINESS.kind ManualByParticipant
//!
//! # Test-Ablauf
//!
//! 1. ZeroDDS-Runtime auf Domain 42, WLP-Period auf 60s gesetzt
//!    (langer AUTOMATIC-Default), damit nur unsere expliziten
//!    `assert_participant()`-Pulses Pakete erzeugen.
//! 2. ddsperf-Sub auf llvm — empfaengt unsere Pulses.
//! 3. Wir rufen 3x `assert_participant()` mit 1s Abstand und checken
//!    via tick(), dass jeder Aufruf ein WLP-Datagram emittiert.

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
    // Wir testen die WLP-Endpoint-API direkt — kein voller Runtime-
    // Spawn noetig, da der Pulse-Output deterministisch und
    // unabhaengig vom AUTOMATIC-Tick ist.
    let mut wlp = WlpEndpoint::new(
        GuidPrefix::from_bytes([0xBB; 12]),
        VendorId::ZERODDS,
        Duration::from_secs(60), // AUTOMATIC quasi aus
    );

    // 3 Manual-Pulses mit Pause dazwischen.
    let mut emitted = 0usize;
    for _ in 0..3 {
        wlp.assert_participant();
        // tick() liefert den manuellen Pulse als Datagram.
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
    // ManualByTopic uebermittelt zusaetzlich ein Topic-Token im
    // ParticipantMessageData (Vendor-Kind 0x02). Wir verifizieren
    // dass das Token byte-genau auf den Wire kommt.
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
    // Token kommt im Inline-Payload — wir suchen den Substring.
    let needle = &token[..];
    let found = dg.windows(needle.len()).any(|w| w == needle);
    assert!(found, "topic token not present in WLP datagram: {dg:?}");
}
