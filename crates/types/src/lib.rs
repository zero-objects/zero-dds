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
//!   (§7.6.3.4 + §7.2.4).
//! - [`qos`] — TypeConsistencyEnforcement + DataRepresentation
//!   (DDS 1.4 §2.2.3).
//! - [`dynamic`] — DynamicType / DynamicData reflection API
//!   (§7.5) incl. TypeObject → DynamicType (§7.6.3). Behind the
//!   `dynamic` feature, which is on by default; see below.
//!
//! ## `dynamic` needs `std`, and the rest of this crate does not
//!
//! [`dynamic`] is the reflective API, and a value of a type known only
//! at run time only means anything if it can reach the wire. What takes
//! it there is `zerodds-wire-codec`, which executes one layout plan
//! against any value representation in both directions — and which is a
//! `std` crate. So the feature that turns [`dynamic`] on takes `std`
//! with it.
//!
//! Everything else here — `type_identifier`, `type_object`, `hash`,
//! `resolve`, `assignability`, `type_matcher`, `qos`, `from_ast` — is
//! `no_std` + `alloc` and stays that way:
//! `--no-default-features --features alloc` builds the type system
//! without the reflective half rather than a reflective half that cannot
//! serialise. Said here rather than found at a link error.
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

// `zerodds-types` mandatorily requires `alloc` — every module uses
// `Vec`/`String`/`BTreeMap` for the TypeObject containers.
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod assignability;
pub mod builder;
#[cfg(feature = "dynamic")]
pub mod dynamic;
pub mod error;
pub mod framing;
pub mod from_ast;
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
pub mod type_object_v1;
pub mod union_inheritance;

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
