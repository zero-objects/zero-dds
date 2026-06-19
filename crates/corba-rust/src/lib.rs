// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-corba-rust`. Safety classification: **STANDARD**.
//!
//! IDL → Rust code generator for CORBA service constructs (interface
//! traits + stubs + skeletons, valuetypes, in phase 2: components,
//! homes, POA bindings).
//!
//! Analogous to `zerodds-idl-cpp` / `-csharp` / `-java` but emits Rust
//! instead of C++/C#/Java. Consumes the `zerodds-corba-codegen` helpers
//! (special-types tables, stub/skeleton templates) and
//! `zerodds-idl-rust::type_map` for DataType mapping.
//!
//! ## Layer position
//!
//! Layer 8 (CORBA stack). Build-time tool, std-only,
//! `forbid(unsafe_code)`.
//!
//! ## Spec source
//!
//! `docs/specs/zerodds-corba-rust-1.0.md` (ZeroDDS vendor spec).
//! Conformance basis: OMG CORBA 3.3 Annex A, OMG IDL4.
//!
//! ## What is emitted
//!
//! | IDL                           | Rust                                              |
//! |-------------------------------|---------------------------------------------------|
//! | `interface I { op(...); };`   | `pub trait I` + `pub struct IStub` + `dispatch_i` |
//! | `attribute T x`               | trait getter `fn x(&self)` + setter if writable   |
//! | `oneway op(...)`              | trait method without reply                        |
//! | `valuetype V { ... };`        | `pub trait V: ValueBase`                          |
//! | `component C` / `home H`      | (phase 2)                                         |
//!
//! ## Public API
//!
//! - [`generate_corba_rust_module`] — AST + options → Rust module code.
//! - [`CorbaRustGenOptions`] — codegen options.
//! - [`error::CorbaRustError`] — error family.
//!
//! Plus the runtime public API that the generated code references:
//! - [`ObjectReference`] — IOR wrapper.
//! - [`CorbaException`] — system/user exception variants.
//! - [`SkeletonResult`] — server dispatch result.
//! - [`ValueBase`] — trait for all valuetype implementations.
//! - [`Servant`] — POA servant marker.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod ami_emit;
pub mod component_emit;
pub mod emitter;
pub mod error;
pub mod interface_emit;
pub mod runtime;
pub mod value_wire;
pub mod valuetype_emit;

pub use emitter::{CorbaRustGenOptions, generate_corba_rust_module};
pub use error::{CorbaRustError, Result};
pub use runtime::{
    AsyncCorbaChannel, AsyncReply, AsyncReplyCallback, ComponentHome, ComponentServant,
    CorbaConnection, CorbaException, IdAssignmentPolicy, IdUniquenessPolicy,
    ImplicitActivationPolicy, LifespanPolicy, ObjectReference, PoaBuilder, RequestProcessingPolicy,
    Servant, ServantRetentionPolicy, SkeletonResult, ThreadPolicy, TypeCode, ValueBase,
    ValueStreamReader, ValueStreamWriter, ValueTagHeader,
};
