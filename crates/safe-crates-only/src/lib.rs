// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `safe-crates-only`. Safety classification: **SAFE**.
//!
//! Meta-crate that aggregates the ZeroDDS **Safe-Subset** — the SAFE-classified,
//! `no_std`-capable crates listed in
//! `docs/architecture/04_safety_by_architecture.md` — so the documented
//! safe-profile gates are real and runnable:
//!
//! ```text
//! # no_std safe-profile build (forwards `safety` + `alloc`, NO std):
//! cargo build -p safe-crates-only --no-default-features --features safety
//! ```
//!
//! It carries no logic of its own; the dependency edges force every Safe-Subset
//! crate into the build under the `safety` profile so a regression (a Safe crate
//! that stops building `no_std`) fails here.
//!
//! `STANDARD`-classified crates (e.g. `zerodds-transport-shm`, `zerodds-dcps`)
//! and the `std`-only build tools (`zerodds-idl*`) are deliberately NOT part of
//! this set, even where an older draft of the architecture doc listed them.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "safety"), allow(unused_extern_crates))]

// Pull every Safe-Subset crate into the build under the safe profile. `pub use`
// of the crate roots is enough to create the dependency edge; the meta-crate
// exposes no API surface of its own.
pub use {
    zerodds_cdr, zerodds_discovery, zerodds_foundation, zerodds_qos, zerodds_rtps,
    zerodds_security, zerodds_sys, zerodds_transport, zerodds_transport_udp, zerodds_types,
    zerodds_xrce_client,
};
