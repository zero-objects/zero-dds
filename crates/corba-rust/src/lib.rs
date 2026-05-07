// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-corba-rust`. Safety classification: **STANDARD**.
//!
//! IDL → Rust Code-Generator fuer CORBA-Service-Konstrukte (Interface-
//! Traits + Stubs + Skeletons, Valuetypes, in Phase-2: Components,
//! Homes, POA-Bindings).
//!
//! Analog zu `zerodds-idl-cpp` / `-csharp` / `-java` aber emittiert Rust
//! statt C++/C#/Java. Konsumiert die `zerodds-corba-codegen`-Helpers
//! (Special-Types-Tabellen, Stub/Skeleton-Templates) und das
//! `zerodds-idl-rust::type_map` fuer DataType-Mapping.
//!
//! ## Schichten-Position
//!
//! Layer 8 (CORBA-Stack). Build-Zeit-Tool, std-only,
//! `forbid(unsafe_code)`.
//!
//! ## Spec-Quelle
//!
//! `docs/specs/zerodds-corba-rust-1.0.md` (ZeroDDS Vendor-Spec).
//! Konformitaetstraeger: OMG CORBA 3.3 Annex-A, OMG IDL4.
//!
//! ## Was wird emittiert
//!
//! | IDL                           | Rust                                              |
//! |-------------------------------|---------------------------------------------------|
//! | `interface I { op(...); };`   | `pub trait I` + `pub struct IStub` + `dispatch_i` |
//! | `attribute T x`               | trait getter `fn x(&self)` + setter wenn writable |
//! | `oneway op(...)`              | trait method ohne Reply                           |
//! | `valuetype V { ... };`        | `pub trait V: ValueBase`                          |
//! | `component C` / `home H`      | (Phase-2)                                         |
//!
//! ## Public API
//!
//! - [`generate_corba_rust_module`] — AST + Optionen → Rust-Modul-Code.
//! - [`CorbaRustGenOptions`] — Codegen-Optionen.
//! - [`error::CorbaRustError`] — Fehler-Familie.
//!
//! Plus die Runtime-Public-API die der generierte Code referenziert:
//! - [`ObjectReference`] — IOR-Wrapper.
//! - [`CorbaException`] — System-/User-Exception-Variants.
//! - [`SkeletonResult`] — Server-Dispatch-Result.
//! - [`ValueBase`] — Trait fuer alle Valuetype-Implementierungen.
//! - [`Servant`] — POA-Servant-Marker.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod component_emit;
pub mod emitter;
pub mod error;
pub mod interface_emit;
pub mod runtime;
pub mod valuetype_emit;

pub use emitter::{CorbaRustGenOptions, generate_corba_rust_module};
pub use error::{CorbaRustError, Result};
pub use runtime::{
    ComponentHome, ComponentServant, CorbaConnection, CorbaException, IdAssignmentPolicy,
    IdUniquenessPolicy, ImplicitActivationPolicy, LifespanPolicy, ObjectReference, PoaBuilder,
    RequestProcessingPolicy, Servant, ServantRetentionPolicy, SkeletonResult, ThreadPolicy,
    TypeCode, ValueBase, ValueStreamReader, ValueStreamWriter, ValueTagHeader,
};
