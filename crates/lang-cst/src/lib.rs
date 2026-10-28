// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-lang-cst`. Safety classification: **SAFE**.
//!
//! Concrete syntax trees for the seventeen target languages. Built,
//! never parsed.
//!
//! Each language gets the tree its own syntax needs. There is no shared
//! target model: with one, the language-specific part does not
//! disappear, it moves into the renderer, which is where it stops being
//! reviewable.
//!
//! ## Spec
//!
//! - `docs/specs/zerodds-lang-cst-1.0.md` §1.
//!
//! ## Layer position
//!
//! Layer 0 of the backend arc. Depends on nothing — not even on the
//! common AST, so that a tree can be built and rendered without a model
//! and a renderer cannot reach back into one.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod ada;
pub mod ada83;
pub mod c;
pub mod common;
pub mod cpp;
pub mod csharp;
pub mod d;
pub mod elixir;
pub mod fsharp;
pub mod go;
pub mod java;
pub mod julia;
pub mod kotlin;
pub mod lua;
pub mod nim;
pub mod node;
pub mod ocaml;
pub mod python;
pub mod rust;
pub mod swift;
pub mod typescript;
pub mod zig;

pub use common::{Doc, Ident, Language, Literal, Path, Visibility};
