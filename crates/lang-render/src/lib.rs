// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-lang-render`. Safety classification: **SAFE**.
//!
//! A language CST goes in, text comes out. Nothing is decided here.
//!
//! The renderer must not consult the common AST, must not branch on
//! anything but the CST node in hand, and must not insert a construct
//! that is not in the tree. A renderer that decided anything would put
//! that decision where no rule diff can see it.
//!
//! ## Spec
//!
//! - `docs/specs/zerodds-lang-cst-1.0.md` §5.
//!
//! ## Layer position
//!
//! Layer 1 of the backend arc — over `zerodds-lang-cst` and nothing else.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod ada;
pub mod ada83;
pub mod c;
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
pub mod writer;
pub mod zig;

pub use writer::Writer;
