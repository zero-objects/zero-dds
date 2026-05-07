// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ZeroDDS Chaos-Engineering Bibliothek.
//!
//! Crate `zerodds-chaos`. Safety classification: **COMFORT** (Test-Tool,
//! kein Runtime-Pfad).
//!
//! # Module
//!
//! * [`proxy`] — In-Process UDP-Chaos-Proxy: injiziert Packet-Loss,
//!   Jitter, Duplicates, Reorder. Plattform-unabhaengig, kein root.
//! * [`tc`] — Linux-`tc qdisc`-Wrapper: nutzt `netem` fuer realistische
//!   Network-Conditions auf einem Interface. Root-Privileg-pflichtig.
//! * [`partition`] — iptables-basiertes Network-Partition zwischen
//!   IP-Gruppen.
//! * [`endpoint_flap`] — toggelt ein Linux-Interface up/down im Takt.
//! * [`prng`] — kleiner xorshift64-Generator fuer reproducible-seeds.
//!
//! # Determinismus
//!
//! Alle Chaos-Operationen seedbar via `--seed`. Gleicher Seed +
//! gleicher Eingabe-Strom = bit-identischer Ausgabe-Strom; Voraussetzung
//! fuer property-test-fitting Pipelines.

#![warn(missing_docs)]

pub mod endpoint_flap;
pub mod partition;
pub mod prng;
pub mod proxy;
pub mod tc;
