// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ZeroDDS Chaos-Engineering library.
//!
//! Crate `zerodds-chaos`. Safety classification: **COMFORT** (Test-Tool,
//! no runtime path).
//!
//! # Module
//!
//! * [`proxy`] — In-Process UDP-Chaos-Proxy: injiziert Packet-Loss,
//!   jitter, duplicates, reorder. Platform-independent, no root.
//! * [`tc`] — Linux `tc qdisc` wrapper: uses `netem` for realistic
//!   network conditions on an interface. Requires root privilege.
//! * [`partition`] — iptables-based network partition between
//!   IP-Gruppen.
//! * [`endpoint_flap`] — toggles a Linux interface up/down in cycles.
//! * [`prng`] — small xorshift64 generator for reproducible seeds.
//!
//! # Determinism
//!
//! All Chaos operations are seedable via `--seed`. Same seed +
//! same input stream = bit-identical output stream; prerequisite
//! for property-test-fitting pipelines.

#![warn(missing_docs)]

pub mod endpoint_flap;
pub mod partition;
pub mod prng;
pub mod proxy;
pub mod tc;
