// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wire-conformance`. Safety classification: **SAFE**.
//!
//! The code the generator writes, compiled.
//!
//! Every other test of the generator reads its output as text. This
//! crate builds it: a signature that does not match the runtime is a
//! compile error here, which is the one place it can be caught before a
//! user's build.
#![forbid(unsafe_code)]

/// The generated declarations and their codecs.
pub mod generated {
    #![allow(clippy::all, clippy::pedantic, missing_docs, dead_code)]

    use zerodds_wire::{Narrowed, ParameterRole, Reader, TryConstruct, WireError, Writer};

    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

pub use zerodds_wire::{Endianness, ParameterRole, Reader, TryConstruct, WireError, Writer};
