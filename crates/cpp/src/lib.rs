// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-cpp`. Safety classification: **STANDARD**.
//!
//! C++17 RAII-wrapper header `dds/dds.hpp` over the `zerodds-c-api`
//! interface. The actual headers live under `include/dds/`;
//! the Rust lib core is just a Cargo container for the C++ test harness.
//!
//! Spec: OMG DDS-PSM-Cxx 1.0 (formal/2013-11-01).
//!
//! ## Layer position
//!
//! Layer 6 — PSMs / Bindings.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! No Rust public API. The caller API lives under `include/dds/dds.hpp`
//! and is consumed via C++ compile against `libzerodds.dylib` (from
//! `zerodds-c-api`).
//!
//! ## Test
//!
//! `cargo test -p zerodds-cpp` builds + links the C++ smoke test
//! (`tests/smoke_dds_psm.cpp`) and runs 10 sub-asserts.

#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // Smoke test: crate compiles and the test harness runs.
    }
}
