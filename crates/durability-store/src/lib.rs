// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS Durability-Service storage abstraction (DDS 1.4 §2.2.3.4/§2.2.3.5).
//!
//! Crate `zerodds-durability-store`. Safety classification: **STANDARD**.
//!
//! This crate is the **storage layer** of the standalone Durability-Service
//! (ADR 0009). It is deliberately free of any DDS/RTPS dependency — the
//! daemon (`zerodds-durability-service`) and adapters
//! (`zerodds-durability-store-{sqlite,file,lakehouse}`) build on top of it.
//!
//! ## Layering (ADR 0009)
//!
//! ```text
//! TieredStore        — memory hot-cache (bounded by a byte budget) over any cold store
//!   └─ DurabilityStore (this trait) — store / query / unregister / cleanup / stats
//!         └─ cold adapter (sqlite / file / lakehouse / in-memory)
//! ```
//!
//! ## Invariants (normative, ADR 0009)
//!
//! 1. **The QoS is the contract.** [`Contract`] (derived from
//!    `DurabilityServiceQosPolicy` + history) is the ONLY thing that bounds
//!    retention. The cold store enforces it (drop-oldest / `OutOfResources`).
//! 2. **The memory budget is not a retention knob.** [`TieredStore`]'s hot
//!    cache only decides how much contracted history is kept hot in RAM; the
//!    rest lives in the cold store and is streamed on read — never capped.
//! 3. **The cold store is authoritative** for the full contracted history.
//!    Reads ([`DurabilityStore::query`]) are paginated ([`Page`]/[`Cursor`]),
//!    never "everything into RAM".

#![forbid(unsafe_code)]

extern crate alloc;

mod contract;
mod error;
mod memory;
mod model;
mod store;
mod tiered;

pub use contract::Contract;
pub use error::{Result, StoreError};
pub use memory::InMemoryStore;
pub use model::{Cursor, DurabilitySample, Page, Selector, StoreStats};
pub use store::DurabilityStore;
pub use tiered::TieredStore;
