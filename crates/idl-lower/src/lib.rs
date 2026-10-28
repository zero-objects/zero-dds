// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-idl-lower`. Safety classification: **SAFE**.
//!
//! Lowers the IDL CST into the common AST: scopes and name binding,
//! resolution, and the IDL rules the model cannot express structurally.
//!
//! A name that cannot be resolved yet becomes a pending reference rather
//! than an error — the model is built for partial knowledge, and forward
//! declarations depend on it.
//!
//! ## Spec
//!
//! - `docs/specs/zerodds-idl-frontend-1.0.md` §4 (lowering), §5
//!   (completion), §6 (validation).
//! - `docs/specs/zerodds-ast-1.0.md` §2 (references), §5 (transactions).
//!
//! ## Layer position
//!
//! Layer 2 of the frontend — over `zerodds-idl-parse` and `zerodds-ast`.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(test)]
extern crate std;

mod annotation;
pub mod complete;
mod expr;
mod lower;
pub mod scope;
pub mod validate;

pub use annotation::verbatim_placement;
pub use complete::hashed_member_id;
pub use lower::{
    HashedIdCounter, HashedNameSpelling, KeyOnUnionDiscriminator, Lowered, NameOfTheEnclosingScope,
    Options, lower, lower_into, lower_into_with, lower_with,
};
pub use scope::ScopeStack;
pub use validate::rule_name;
