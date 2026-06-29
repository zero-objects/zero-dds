// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-types`. Safety classification: **SAFE**.
//!
//! OMG XTypes 1.3 Type-System — TypeIdentifier, TypeObject, Compatibility.
//! Pure-Rust no_std + alloc, `forbid(unsafe_code)`.
//!
//! ## Spec
//!
//! - **OMG XTypes 1.3** §7.3.1 (TypeIdentifier), §7.3.4 (TypeIdentifier-Union)
//! - **OMG XTypes 1.3** §7.3.4 (Minimal + Complete TypeObject)
//! - **OMG XTypes 1.3** §7.3.5 (TypeInformation + Dependencies)
//! - **OMG XTypes 1.3** §7.3.6 (TypeLookup Service IDL)
//! - **OMG XTypes 1.3** §7.2.4 (assignability + compatibility rules)
//! - **OMG XTypes 1.3** §7.6.3 (DynamicType ↔ TypeObject Bridge)
//! - **OMG XTypes 1.3** §7.6.8 (KeyHash + EquivalenceHash via MD5)
//! - **OMG DDS 1.4** §2.2.3 (TypeConsistencyEnforcement QoS-Policy)
//!
//! ## Layer position
//!
//! Layer 1 — primitives. Direct dependents: `zerodds-discovery`,
//! `zerodds-dcps`, `zerodds-idl`, `zerodds-rpc`, `zerodds-xml`.
//! Architecture + RFC: `internal/rfcs/0004-xtypes-integration.md`.
//!
//! ## Public-API modules
//!
//! - [`type_identifier`] — TypeIdentifier union (§7.3.4.2).
//! - [`type_object`] — minimal + complete TypeObject (§7.3.4).
//! - [`type_information`] — TypeInformation + dependencies (§7.3.5).
//! - [`type_lookup`] — getTypes / getTypeDependencies IDL (§7.3.6).
//! - [`builder`] — programmatic builder for all kinds.
//! - [`hash`] — MD5 → 14-byte EquivalenceHash (§7.3.1.2.1).
//! - [`resolve`] — TypeRegistry + alias resolution + DoS caps.
//! - [`assignability`] — type-compatibility rules (§7.2.4).
//! - [`type_matcher`] — writer↔reader matching with TCE policy
//!   (§7.6.3.7 + §7.2.4).
//! - [`qos`] — TypeConsistencyEnforcement + DataRepresentation
//!   (DDS 1.4 §2.2.3).
//! - [`dynamic`] — DynamicType / DynamicData reflection API
//!   (§7.5) incl. DynamicType ↔ TypeObject bridge (§7.6.3).
//!
//! ## Wiring status
//!
//! `assignability` and `type_matcher` are **public API** for end-user
//! code that does its own type-compatibility checks outside the DDS
//! discovery pipeline (e.g. bridge implementations, custom schema
//! registries). The ZeroDDS discovery pipeline currently matches by
//! `type_name` (DDS 1.4 §2.2.3 default path); full XTypes 1.3 §7.6
//! TypeIdentifier-aware discovery is its own cross-layer architecture
//! epic (TypeIdentifier constants in codegen + SEDP propagation +
//! TypeRegistry shared state).

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

// `zerodds-types` mandatorily requires `alloc` — all modules use
// `Vec`/`String`/`BTreeMap` for TypeObject containers. `zerodds-cdr` with
// the `alloc` feature is a mandatory dep, so `extern crate alloc` is
// always available.
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod assignability;
pub mod builder;
pub mod dynamic;
pub mod error;
pub mod hash;
pub mod qos;
pub mod resolve;
pub mod type_identifier;
pub mod type_information;
pub mod type_lookup;
pub mod type_matcher;
#[cfg(test)]
mod type_matcher_vectors;
pub mod type_object;

pub use error::TypeCodecError;
pub use hash::{
    compute_complete_hash, compute_hash, compute_minimal_hash, to_hashed_type_identifier,
};
pub use type_identifier::{
    EquivalenceHash, EquivalenceKind, PlainCollectionHeader, PrimitiveKind,
    StronglyConnectedComponentId, TypeIdentifier,
};
pub use type_information::{
    TypeIdentifierWithDependencies, TypeIdentifierWithSize, TypeInformation,
};
pub use type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};
