// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CosNaming 1.3 — Naming-Service.
//!
//! Crate `zerodds-corba-cosnaming`. Safety classification: **STANDARD**.
//! Spec OMG CosNaming 1.3 (`formal/2004-10-03`).
//!
//! Voller NamingContext + NamingContextExt + Stringified-Name +
//! corbaname URL support. In-memory implementation, fully
//! spec-compliant incl. all 5 exception classes.
//!
//! ## Example
//!
//! ```
//! use zerodds_corba_cosnaming::NameComponent;
//! let nc = NameComponent { id: "obj".into(), kind: "Object".into() };
//! assert_eq!(nc.id, "obj");
//! assert_eq!(nc.kind, "Object");
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod context;
pub mod error;
pub mod name;
pub mod stringified;

pub use context::{Binding, BindingType, NamingContext, ObjectRef};
pub use error::{NamingError, NotFoundReason};
pub use name::{Name, NameComponent};
pub use stringified::{name_to_string, string_to_name};
