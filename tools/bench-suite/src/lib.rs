// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Shared helpers for the v1.2 benchmark suite.
//!
//! Crate `zerodds-bench-suite`. Safety classification: **COMFORT**.
//! Test-/Bench-only, keine Runtime-Abhaengigkeit. Kein unsafe,
//! aber auch kein formaler no-panic-Kontrakt fuer die Harness-
//! Helfer — `make_payload` kann bei OOM panic'en (Bench laeuft
//! auf einer dedizierten Maschine).
//!
//! # Scope
//!
//! Zitierfaehige Perf-Messungen brauchen eine **standardisierte
//! Harness**: gleiche Payload-Erzeugung, gleiches Timing-Pattern,
//! dokumentierbares Reproduce-Rezept. Diese Crate bietet die
//! Bausteine:
//!
//! - [`PAYLOAD_SIZES`] — kanonische Payload-Achse (9 Groessen von
//!   32 B bis 4 MB).
//! - `TransportDriver` — Trait fuer einheitlichen Sender-
//!   Lifecycle (prepare → warmup → measure).
//! - `make_payload` — deterministisch generierte Bytes (kein
//!   Allocator-Noise, kein RNG-Overhead im Hot-Path).
//!
//! Die Bench-Targets (`benches/*.rs`) nutzen diese Bausteine +
//! `criterion`. Reports landen als JSON in `target/criterion/` und
//! werden ueber `internal/perf/baseline-*.md` referenziert.

#![warn(missing_docs)]

/// Kanonische Payload-Groessen-Achse. Zwei Skala-Dekaden pro Stufe,
/// bis 4 MiB (genug fuer high-res camera frames nach Fragmentation).
///
/// **Designentscheidung** — wir messen diese 9 Punkte immer zusammen,
/// damit Reports zwischen Runs direkt vergleichbar sind. Ad-hoc-Groessen
/// gehoeren nicht in die Suite; sie fuehren zu inkonsistenten Plots.
pub const PAYLOAD_SIZES: &[usize] = &[
    32, 128, 1_024,     // 1 KiB
    4_096,     // 4 KiB
    16_384,    // 16 KiB
    65_536,    // 64 KiB — UDP-DGRAM-cap, sinnvolle Bruchkante
    262_144,   // 256 KiB
    1_048_576, // 1 MiB
    4_194_304, // 4 MiB
];

/// Groesste Payload in der Achse — fuer Vec-Preallocation im Fixture.
pub const MAX_PAYLOAD: usize = 4_194_304;

/// Erzeuge einen deterministischen Payload. LCG-basiert, damit der
/// Content nicht all-zero ist (einige Transports/OS haben fastpath
/// fuer zero-pages, was Baselines verfaelscht).
#[must_use]
pub fn make_payload(size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    let mut state: u64 = 0xDEAD_BEEF_CAFE_BABE;
    for _ in 0..size {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        out.push((state >> 33) as u8);
    }
    out
}

/// Bench-relevante Payload-Groessen mit human-readable Label.
/// Fuer BenchmarkId-Parameter.
#[must_use]
pub fn size_label(size: usize) -> String {
    if size < 1024 {
        format!("{size}B")
    } else if size < 1024 * 1024 {
        format!("{}KiB", size / 1024)
    } else {
        format!("{}MiB", size / (1024 * 1024))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn payload_sizes_are_sorted_ascending() {
        for pair in PAYLOAD_SIZES.windows(2) {
            assert!(pair[0] < pair[1]);
        }
    }

    #[test]
    fn max_payload_matches_last_entry() {
        assert_eq!(MAX_PAYLOAD, *PAYLOAD_SIZES.last().unwrap());
    }

    #[test]
    fn make_payload_is_deterministic() {
        let a = make_payload(128);
        let b = make_payload(128);
        assert_eq!(a, b);
    }

    #[test]
    fn make_payload_is_not_all_zero() {
        let p = make_payload(1024);
        assert!(p.iter().any(|&b| b != 0));
    }

    #[test]
    fn size_label_formats() {
        assert_eq!(size_label(32), "32B");
        assert_eq!(size_label(1024), "1KiB");
        assert_eq!(size_label(1_048_576), "1MiB");
    }
}
