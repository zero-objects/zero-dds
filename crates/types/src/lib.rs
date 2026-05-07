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
//! - **OMG XTypes 1.3** §7.2.4 (Assignability + Compatibility-Regeln)
//! - **OMG XTypes 1.3** §7.6.3 (DynamicType ↔ TypeObject Bridge)
//! - **OMG XTypes 1.3** §7.6.8 (KeyHash + EquivalenceHash via MD5)
//! - **OMG DDS 1.4** §2.2.3 (TypeConsistencyEnforcement QoS-Policy)
//!
//! ## Schichten-Position
//!
//! Layer 1 — Primitives. Direkte Abhängige: `zerodds-discovery`,
//! `zerodds-dcps`, `zerodds-idl`, `zerodds-rpc`, `zerodds-xml`.
//! Architektur + RFC: `docs/rfcs/0004-xtypes-integration.md`.
//!
//! ## Public-API-Module
//!
//! - [`type_identifier`] — TypeIdentifier-Union (§7.3.4.2).
//! - [`type_object`] — Minimal + Complete TypeObject (§7.3.4).
//! - [`type_information`] — TypeInformation + Dependencies (§7.3.5).
//! - [`type_lookup`] — getTypes / getTypeDependencies IDL (§7.3.6).
//! - [`builder`] — programmatischer Builder für alle Kinds.
//! - [`hash`] — MD5 → 14-byte EquivalenceHash (§7.3.1.2.1).
//! - [`resolve`] — TypeRegistry + Alias-Resolution + DoS-Caps.
//! - [`assignability`] — Type-Compatibility-Regeln (§7.2.4).
//! - [`type_matcher`] — Writer↔Reader-Matching mit TCE-Policy
//!   (§7.6.3.7 + §7.2.4).
//! - [`qos`] — TypeConsistencyEnforcement + DataRepresentation
//!   (DDS 1.4 §2.2.3).
//! - [`dynamic`] — DynamicType / DynamicData Reflection-API
//!   (§7.5) inkl. DynamicType ↔ TypeObject Bridge (§7.6.3).
//!
//! ## Wiring-Status
//!
//! `assignability` und `type_matcher` sind **Public-API** für End-User-
//! Code, der eigene Type-Compatibility-Checks außerhalb der DDS-Discovery-
//! Pipeline durchführt (z.B. Bridge-Implementations, Custom-Schema-
//! Registries). Die ZeroDDS-Discovery-Pipeline matched aktuell per
//! `type_name` (DDS 1.4 §2.2.3 Default-Path); volle XTypes-1.3-§7.6
//! TypeIdentifier-aware Discovery ist eine eigene cross-layer Architektur-
//! Epic (TypeIdentifier-Konstanten in Codegen + SEDP-Propagation +
//! TypeRegistry-shared-state).

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

// `zerodds-types` setzt `alloc` zwingend voraus — alle Module nutzen
// `Vec`/`String`/`BTreeMap` fuer TypeObject-Container. `zerodds-cdr` mit
// `alloc`-Feature ist mandatory dep, also ist `extern crate alloc`
// immer verfuegbar.
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
