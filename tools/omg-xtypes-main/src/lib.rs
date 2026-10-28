// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-omg-xtypes-main`. Safety classification: **TOOLING**.
//!
//! ZeroDDS reference client for the OMG `dds-xtypes` interoperability
//! test suite (`github.com/omg-dds/dds-xtypes`), as a library beside the
//! `xtypes_main` binary.
//!
//! BUILD + LOCAL-VALIDATE phase artifact. Nothing here has been
//! submitted upstream — no CLA PR, no source PR, no release asset, no
//! push to `github.com/omg-dds`. That step is Sandra's.
//!
//! The binary is a thin shell over [`cli`] and [`shape_type`]; the
//! library exists so the 25 generated types and the dispatch over them
//! can be measured from `tests/` rather than only exercised through a
//! process. `tests/compatibility_matrix.rs` is the reason: it holds our
//! assignability implementation against the suite's own
//! `compatibility_matrix.xlsx`, which is the thing this suite is for.

#![forbid(unsafe_code)]

pub mod cli;
pub mod listeners;
pub mod shape_type;
