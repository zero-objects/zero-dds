// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-codegen`. Safety classification: **STANDARD**.
//!
//! OMG CORBA 3.3 Annex-A.1 IDL-Mapping-Codegen-Helpers — pure-Rust
//! `no_std + alloc`, `forbid(unsafe_code)`. Liefert Tabellen + Helper,
//! die [`zerodds-idl-cpp`], [`zerodds-idl-csharp`] und
//! [`zerodds-idl-java`] zur Erzeugung CORBA-Stub-/Skeleton-Codes
//! konsumieren.
//!
//! Spec: OMG CORBA 3.3 Part 1 Annex A (IDL-Type-Mappings) +
//! `formal/2008-01-09` IDL-to-C++ + `formal/2008-01-04` IDL-to-Java.
//!
//! ## Schichten-Position
//!
//! Layer 8 — CORBA-Stack. Substrat fuer die drei OMG-PSM-Codegen-
//! Crates ([`zerodds-idl-cpp`] / [`zerodds-idl-csharp`] /
//! [`zerodds-idl-java`]).
//!
//! [`zerodds-idl-cpp`]: ../zerodds_idl_cpp/index.html
//! [`zerodds-idl-csharp`]: ../zerodds_idl_csharp/index.html
//! [`zerodds-idl-java`]: ../zerodds_idl_java/index.html
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`SpecialType`] / [`TargetLanguage`] / [`language_mapping`] —
//!   13 Annex-A.1 Special-Types (Object, ValueBase, AbstractBase,
//!   NativeRef, TypeCode, any, sequence-of-any, string, wstring,
//!   Time, fixed, ULongLong, LongDouble) mit Sprach-Mappings fuer
//!   C++ / C# / Java.
//! - [`StubOp`] / [`render_stub_op`] — Client-Stub-Template (sendet
//!   GIOP-Request, erwartet Reply mit ReplyStatus-Switch).
//! - [`SkeletonOp`] / [`render_skeleton_dispatch`] — Server-Side-
//!   Dispatch-Template (Operation-Name-Switch).
//! - [`build_repository_id`] — Repository-ID-Builder (Spec §10.7.3.1)
//!   fuer IDL-`module`/`interface`-Pfade.
//!
//! ## Beispiel
//!
//! ```rust
//! use zerodds_corba_codegen::build_repository_id;
//!
//! let id = build_repository_id(&["MyModule"], "MyInterface", 1, 0);
//! assert_eq!(id, "IDL:MyModule/MyInterface:1.0");
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod repository_id;
pub mod skeleton;
pub mod special_types;
pub mod stub;

pub use repository_id::build_repository_id;
pub use skeleton::{SkeletonOp, render_skeleton_dispatch};
pub use special_types::{SpecialType, TargetLanguage, language_mapping};
pub use stub::{StubOp, render_stub_op};
