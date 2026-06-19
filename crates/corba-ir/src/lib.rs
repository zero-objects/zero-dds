// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CORBA 3.3 Part 1 §14 — Interface Repository (IR).
//!
//! Crate `zerodds-corba-ir`. Safety classification: **STANDARD**.
//! `no_std + alloc`, `forbid(unsafe_code)`.
//!
//! Full IR stack: TypeCode (all 32 TCKinds — `tk_null` … `tk_local_interface`),
//! Repository with a containment hierarchy (`Container`/`Definition`/`Module`),
//! `DefinitionKind` (`dk_*` constants), structured `RepositoryId`
//! with a spec §10.7.3.1 format parser/builder.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`RepositoryId`] — `IDL:<scoped>:<major>.<minor>` parser/builder.
//! - [`TypeCode`] / [`TcKind`] / [`UnionMember`] — TypeCode model.
//! - [`Repository`] / [`Container`] / [`Definition`] / [`Module`] — IR hierarchy.
//! - [`DefinitionKind`] — `dk_None` … `dk_LocalInterface`.
//! - [`IrError`] / [`IrResult`] — repository error.
//!
//! ## Consumers
//!
//! - [`zerodds_corba_poa`] uses `RepositoryId::parse` for typed
//!   validation of servant interfaces (spec §11.3.5.20.4 `_is_a`).
//! - External CORBA applications consume the IR via IIOP/IOR.
//!
//! ## Example
//!
//! ```
//! use zerodds_corba_ir::RepositoryId;
//! let r = RepositoryId::parse("IDL:omg.org/CosNaming/NamingContext:1.0").unwrap();
//! assert_eq!(r.scoped_name, "omg.org/CosNaming/NamingContext");
//! assert_eq!(r.major, 1);
//! assert_eq!(r.minor, 0);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod definition_kind;
pub mod error;
pub mod repository;
pub mod repository_id;
pub mod type_code;

pub use definition_kind::DefinitionKind;
pub use error::{IrError, IrResult};
pub use repository::{
    AttributeMode, Container, DefDetails, Definition, Module, OperationMode, ParameterDef,
    ParameterMode, Repository,
};
pub use repository_id::RepositoryId;
pub use type_code::{TcKind, TypeCode, UnionMember};
