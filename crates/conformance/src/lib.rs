// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Conformance-Test-Vector-Runner.
//!
//! Crate `zerodds-conformance`. Safety classification: **STANDARD**.
//!
//! Pro externer Conformance-Suite ein Modul, das die Spec-Test-
//! Vektoren als Konstanten haelt und gegen die Production-
//! Implementations laufen laesst. Damit ist die Konformitaet im
//! Workspace nachweisbar ohne externe CI-Tool-Setup-Abhaengigkeit
//! — externe Tools (Autobahn-Server, h2spec) bleiben optional in
//! `live-interop`-Job.
//!
//! # Module
//!
//! * [`autobahn_ws`] — RFC 6455 + RFC 7692 (WebSocket).
//! * [`oasis_mqtt`] — OASIS MQTT-5.0 Spec §3 + §4.
//! * [`h2spec_grpc`] — RFC 7540/7541 (HTTP/2 + HPACK) + gRPC-interop.
//! * [`coap_plugtest`] — IETF-Plugtest (RFC 7252 + 7641 + 7959).
//! * [`dds_xml_xvendor`] — DDS-XML 1.0 Cross-Vendor + W3C-XSD.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod autobahn_ws;
pub mod coap_plugtest;
pub mod dds_xml_xvendor;
pub mod h2spec_grpc;
pub mod oasis_mqtt;

/// Ergebnis eines einzelnen Conformance-Test-Cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseResult {
    /// Test bestanden.
    Pass,
    /// Test fehlgeschlagen, mit Begruendung.
    Fail(alloc::string::String),
    /// Test geskipped (z.B. Spec optional + Feature-Flag aus).
    Skip(&'static str),
}

impl CaseResult {
    /// `true` wenn `Pass` oder `Skip`.
    #[must_use]
    pub fn is_acceptable(&self) -> bool {
        !matches!(self, Self::Fail(_))
    }
}

/// Ein einzelner Test-Case (Name + Run-Funktion).
pub struct TestCase {
    /// Name (typisch Spec-§-Referenz).
    pub name: &'static str,
    /// Run-Funktion.
    pub run: fn() -> CaseResult,
}

/// Liefert ein Reporting-Tupel `(passed, skipped, failed)`.
#[must_use]
pub fn run_suite(suite: &[TestCase]) -> (usize, usize, usize) {
    let mut pass = 0usize;
    let mut skip = 0usize;
    let mut fail = 0usize;
    for c in suite {
        match (c.run)() {
            CaseResult::Pass => pass += 1,
            CaseResult::Skip(_) => skip += 1,
            CaseResult::Fail(_) => fail += 1,
        }
    }
    (pass, skip, fail)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn case_result_pass_acceptable() {
        assert!(CaseResult::Pass.is_acceptable());
        assert!(CaseResult::Skip("foo").is_acceptable());
        assert!(!CaseResult::Fail("bar".into()).is_acceptable());
    }

    #[test]
    fn run_suite_reports_counts() {
        fn p() -> CaseResult {
            CaseResult::Pass
        }
        fn s() -> CaseResult {
            CaseResult::Skip("x")
        }
        fn f() -> CaseResult {
            CaseResult::Fail("y".into())
        }
        let suite = [
            TestCase { name: "a", run: p },
            TestCase { name: "b", run: s },
            TestCase { name: "c", run: f },
        ];
        let (p, s, f) = run_suite(&suite);
        assert_eq!((p, s, f), (1, 1, 1));
    }
}
