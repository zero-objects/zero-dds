// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The gap between HDDS's generator and its runtime, and nothing else.
//!
//! # Why this exists
//!
//! HDDS's parts are versioned apart. `hddsgen` 1.2.0 emits code against
//! a runtime that is not published:
//!
//! | what the generated code needs | `hdds` 1.1.2 (crates.io) | `ci-v1.1.5` branch |
//! |---|---|---|
//! | `Cdr2Encode::encode_cdr2_le_at` | trait exists, method does not | trait does not exist |
//! | `hdds::CdrVersion` | absent | — |
//! | `hdds::api::DDS` | absent | — |
//! | `hdds::xtypes::*`, `hdds::core::types::*` | present | — |
//!
//! So generated code compiles against nothing that can be fetched, and
//! `wireprobe-hdds` had no runtime to build with.
//!
//! # What this is and is not
//!
//! This crate is named `hdds` because hddsgen's output refers to
//! `::hdds::api::DDS` by absolute path. It **re-exports the published
//! crate** for everything that crate has, and declares only what the
//! generator expects and the runtime does not provide. There is no
//! encoding logic here and there must never be: the octets
//! `wireprobe-hdds` reports come from the inherent `encode_xcdr1_le` and
//! `encode_xcdr2_le` methods in hddsgen's own output, which is compiled
//! verbatim.
//!
//! The consequence to keep in view: this makes HDDS's **generated
//! serialiser** measurable, not its runtime. A defect that lives in the
//! runtime rather than in generated code is invisible to this probe.

#![allow(dead_code)]

// What generated code actually refers to, re-exported from the published
// runtime so a `::hdds::xtypes::CompleteStructType` is HDDS's own type
// and not a replica of it.
//
// Deliberately only these three. Re-exporting the whole crate pulls in
// modules behind feature flags — `rpc` is `#[cfg(feature = "rpc")]` —
// and a shim that fails to build over a module no generated file names
// would be an obstacle rather than a bridge. Anything else a future
// generator reaches for shows up as an unresolved path naming itself.
pub use hdds_upstream::{CdrError, core, xtypes};

/// Which encoding version a call means.
///
/// Expected by hddsgen 1.2.0 output; absent from `hdds` 1.1.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdrVersion {
    /// XCDR version 1.
    Xcdr1,
    /// XCDR version 2.
    Xcdr2,
}

/// Encode to XCDR2 little-endian.
///
/// Declared here rather than taken from the runtime because 1.1.2's
/// trait of this name lacks `encode_cdr2_le_at`, which the generated
/// code implements.
pub trait Cdr2Encode {
    /// Write the value at the start of `dst`, returning the length.
    ///
    /// # Errors
    /// Whatever the generated encoder returns.
    fn encode_cdr2_le(&self, dst: &mut [u8]) -> Result<usize, CdrError>;

    /// The largest encoding this value can produce.
    fn max_cdr2_size(&self) -> usize;

    /// Write the value at `offset`, advancing it.
    ///
    /// # Errors
    /// Whatever the generated encoder returns.
    fn encode_cdr2_le_at(&self, dst: &mut [u8], offset: &mut usize) -> Result<(), CdrError>;
}

/// Decode from XCDR2 little-endian.
pub trait Cdr2Decode: Sized {
    /// Read a value from the start of `src`, with the length consumed.
    ///
    /// # Errors
    /// Whatever the generated decoder returns.
    fn decode_cdr2_le(src: &[u8]) -> Result<(Self, usize), CdrError>;

    /// Read a value at `offset`, advancing it.
    ///
    /// # Errors
    /// As [`Self::decode_cdr2_le`].
    fn decode_cdr2_le_at(src: &[u8], offset: &mut usize) -> Result<Self, CdrError>;
}

/// The surface hddsgen's `DDS` implementation is written against.
///
/// Absent from `hdds` 1.1.2 entirely.
pub mod api {
    /// What a runtime call answers with.
    pub type Result<T> = core::result::Result<T, crate::CdrError>;

    /// A type the runtime can carry.
    ///
    /// `Sized` because `decode` returns `Self` by value.
    pub trait DDS: Sized {
        /// The type's metadata.
        fn type_descriptor() -> &'static crate::core::types::TypeDescriptor;

        /// Encode at the requested version, returning the length.
        ///
        /// The generated body dispatches to the inherent
        /// `encode_xcdr1_le` / `encode_xcdr2_le`, which is where the
        /// octets are decided.
        ///
        /// # Errors
        /// Whatever the inherent encoder returns.
        fn encode(&self, buf: &mut [u8], version: crate::CdrVersion) -> Result<usize>;

        /// Decode at the requested version.
        ///
        /// # Errors
        /// Whatever the inherent decoder returns.
        fn decode(buf: &[u8], version: crate::CdrVersion) -> Result<Self>;

        /// The type's `CompleteTypeObject`, when the generator emitted
        /// one.
        fn get_type_object() -> Option<crate::xtypes::CompleteTypeObject>;

        /// The instance's key hash — sixteen octets, zeroed when the
        /// type declares no `@key`.
        fn compute_key(&self) -> [u8; 16];

        /// Whether the type declares a key at all.
        fn has_key() -> bool;
    }
}
