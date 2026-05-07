// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-cpp`. Safety classification: **STANDARD**.
//!
//! C++17-RAII-Wrapper-Header `dds/dds.hpp` ueber das `zerodds-c-api`-
//! Interface. Die eigentlichen Headers leben unter `include/dds/`,
//! der Rust-Lib-Kern ist nur Cargo-Container fuer den C++-Test-Harness.
//!
//! Spec: OMG DDS-PSM-Cxx 1.0 (formal/2013-11-01).
//!
//! ## Schichten-Position
//!
//! Layer 6 — PSMs / Bindings.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! Keine Rust-Public-API. Caller-API lebt unter `include/dds/dds.hpp`
//! und wird via C++-Compile gegen `libzerodds.dylib` (aus
//! `zerodds-c-api`) konsumiert.
//!
//! ## Test
//!
//! `cargo test -p zerodds-cpp` baut + linkt den C++-Smoke-Test
//! (`tests/smoke_dds_psm.cpp`) und fuehrt 10 Sub-Asserts aus.

#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // Smoke-Test: Crate kompiliert und Testharness laeuft.
    }
}
