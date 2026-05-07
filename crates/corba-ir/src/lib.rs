// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CORBA 3.3 Part 1 §14 — Interface Repository (IR).
//!
//! Crate `zerodds-corba-ir`. Safety classification: **STANDARD**.
//! `no_std + alloc`, `forbid(unsafe_code)`.
//!
//! Voller IR-Stack: TypeCode (alle 32 TCKinds — `tk_null` … `tk_local_interface`),
//! Repository mit Containment-Hierarchie (`Container`/`Definition`/`Module`),
//! `DefinitionKind` (`dk_*`-Konstanten), strukturierte `RepositoryId`
//! mit Spec-§10.7.3.1-Format-Parser/Builder.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`RepositoryId`] — `IDL:<scoped>:<major>.<minor>` Parser/Builder.
//! - [`TypeCode`] / [`TcKind`] / [`UnionMember`] — TypeCode-Modell.
//! - [`Repository`] / [`Container`] / [`Definition`] / [`Module`] — IR-Hierarchie.
//! - [`DefinitionKind`] — `dk_None` … `dk_LocalInterface`.
//! - [`IrError`] / [`IrResult`] — Repository-Fehler.
//!
//! ## Konsumenten
//!
//! - [`zerodds_corba_poa`] verwendet `RepositoryId::parse` zur typisierten
//!   Validierung von Servant-Interfaces (Spec §11.3.5.20.4 `_is_a`).
//! - Externe CORBA-Anwendungen konsumieren den IR via IIOP/IOR.
//!
//! ## Beispiel
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
pub use repository::{Container, Definition, Module, Repository};
pub use repository_id::RepositoryId;
pub use type_code::{TcKind, TypeCode, UnionMember};
